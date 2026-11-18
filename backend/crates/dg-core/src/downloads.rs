//! Download subsystem (sections 5.2 to 5.4): job model, probing, segmented Range downloads,
//! retry policy, pause/resume/cancel, queue with a concurrency limit, and completion into the
//! library. Media links go through the extractor host (`extractor.rs`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dg_types::ErrorCode;
use futures_util::StreamExt;
use reqwest::header::{CONTENT_DISPOSITION, CONTENT_TYPE, ETAG, LAST_MODIFIED, RANGE};
use serde_json::{json, Value};

use crate::db::{JobRow, SegmentRow};
use crate::engine::{err, internal, lock, Engine, Res};
use crate::util::{self, now_ms, now_s};

pub const CTRL_RUN: u8 = 0;
pub const CTRL_PAUSE: u8 = 1;
pub const CTRL_CANCEL: u8 = 2;

const MIB: u64 = 1024 * 1024;
const PER_HOST_CAP: u64 = 8;

/// One byte range of a download and its progress.
pub struct Seg {
    pub idx: i64,
    pub start: u64,
    pub end: u64,
    pub done: AtomicU64,
}

pub struct Job {
    pub id: String,
    pub ctrl: AtomicU8,
    pub running: AtomicBool,
    /// Set once the job has been finished as cancelled, so a second `finish_cancel` (the API call
    /// and the driver both reach it) is a no-op instead of emitting the event twice.
    pub cancel_done: AtomicBool,
    pub state: Mutex<String>,
    pub error: Mutex<Option<String>>,
    pub url: Mutex<String>,
    pub name: Mutex<String>,
    pub kind: String,
    pub quality: Mutex<Option<String>>,
    pub source_page: Option<String>,
    pub total: AtomicU64,
    pub segs: Mutex<Vec<Arc<Seg>>>,
    pub validator: Mutex<Option<String>>,
    pub dest_dir: Mutex<String>,
    pub created_ms: i64,
    pub priority: AtomicI64,
    pub bps: AtomicU64,
    pub prev_done: AtomicU64,
    pub prev_ms: AtomicI64,
    pub finished_ms: AtomicI64,
    pub retry_count: AtomicU64,
    pub range_ok: AtomicBool,
    pub resolved_url: Mutex<Option<String>>,
}

impl Job {
    pub fn state(&self) -> String {
        lock(&self.state).clone()
    }

    pub fn set_state(&self, s: &str) {
        *lock(&self.state) = s.to_string();
    }

    pub fn done(&self) -> u64 {
        lock(&self.segs).iter().map(|s| s.done.load(Ordering::SeqCst)).sum()
    }

    fn from_row(row: &JobRow, segs: Vec<SegmentRow>) -> Job {
        let total = row.total.max(0) as u64;
        let segs: Vec<Arc<Seg>> = segs
            .into_iter()
            .map(|s| Arc::new(Seg { idx: s.idx, start: s.start as u64, end: s.end as u64, done: AtomicU64::new(s.done as u64) }))
            .collect();
        let state = match row.state.as_str() {
            "queued" | "probing" | "downloading" => "queued".to_string(),
            other => other.to_string(),
        };
        Job {
            id: row.id.clone(),
            ctrl: AtomicU8::new(CTRL_RUN),
            running: AtomicBool::new(false),
            cancel_done: AtomicBool::new(false),
            state: Mutex::new(state),
            error: Mutex::new(row.error.clone()),
            url: Mutex::new(row.url.clone()),
            name: Mutex::new(row.filename.clone()),
            kind: row.kind.clone(),
            quality: Mutex::new(row.quality.clone()),
            source_page: row.source_page.clone(),
            total: AtomicU64::new(total),
            segs: Mutex::new(segs),
            validator: Mutex::new(row.validator.clone()),
            dest_dir: Mutex::new(row.dest_dir.clone()),
            created_ms: row.created_at * 1000,
            priority: AtomicI64::new(row.priority),
            bps: AtomicU64::new(0),
            prev_done: AtomicU64::new(0),
            prev_ms: AtomicI64::new(now_ms()),
            finished_ms: AtomicI64::new(0),
            retry_count: AtomicU64::new(row.retry_count as u64),
            range_ok: AtomicBool::new(true),
            resolved_url: Mutex::new(None),
        }
    }

    fn json(&self) -> Value {
        let total = self.total.load(Ordering::SeqCst);
        let done = self.done();
        json!({
            "id": self.id,
            "name": lock(&self.name).clone(),
            "kind": kind_label(&lock(&self.name).clone(), &self.kind),
            "state": self.state(),
            "url": lock(&self.url).clone(),
            "total": total,
            "done": done,
            "bps": self.bps.load(Ordering::SeqCst),
            "quality": lock(&self.quality).clone().unwrap_or_else(|| "Direct file".into()),
            "priority": self.priority.load(Ordering::SeqCst),
            "retryCount": self.retry_count.load(Ordering::SeqCst),
            "error": lock(&self.error).clone(),
            "createdAt": self.created_ms,
            "sourcePage": self.source_page,
        })
    }
}

/// Maps a file name to the UI's kind words (`video`, `audio`, `document`, `app`, ...).
fn kind_label(name: &str, fallback: &str) -> String {
    let k = util::kind_from_name(name);
    if k == "other" && !fallback.is_empty() && fallback != "media" {
        fallback.to_string()
    } else if k == "other" {
        "other".into()
    } else {
        k.to_string()
    }
}

/// Segment count from §5.3 tiers, capped by settings and the per-host connection cap.
pub fn plan_segment_count(total: u64, settings_cap: u32) -> u64 {
    let tier = if total < MIB {
        1
    } else if total < 20 * MIB {
        4
    } else if total < 1024 * MIB {
        8
    } else {
        16
    };
    tier.min(settings_cap as u64).min(PER_HOST_CAP).max(1)
}

pub fn split_ranges(total: u64, n: u64) -> Vec<(u64, u64)> {
    if total == 0 {
        return vec![];
    }
    let n = n.clamp(1, total);
    let base = total / n;
    let rem = total % n;
    let mut out = Vec::new();
    let mut cursor = 0;
    for i in 0..n {
        let len = base + if i < rem { 1 } else { 0 };
        out.push((cursor, cursor + len));
        cursor += len;
    }
    out
}

pub fn priority_value(p: &str) -> Option<i64> {
    match p {
        "high" => Some(2),
        "normal" => Some(1),
        "low" => Some(0),
        _ => None,
    }
}

/* ---------- probing ---------- */

#[derive(Debug, Clone)]
pub struct Probe {
    pub total: Option<u64>,
    pub accepts_ranges: bool,
    pub validator: Option<String>,
    pub filename: String,
    pub content_type: String,
    pub final_url: String,
}

fn filename_from_disposition(v: &str) -> Option<String> {
    for part in v.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("filename*=") {
            let raw = rest.trim_matches('"');
            let raw = raw.rsplit("''").next().unwrap_or(raw);
            return Some(percent_decode(raw));
        }
        if let Some(rest) = part.strip_prefix("filename=") {
            return Some(rest.trim_matches('"').to_string());
        }
    }
    None
}

pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Some(v) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

pub fn filename_from_url(url: &str) -> Option<String> {
    let no_query = url.split(['?', '#']).next()?;
    let last = no_query.rsplit('/').next()?;
    let decoded = percent_decode(last);
    if decoded.is_empty() || !decoded.contains('.') {
        None
    } else {
        Some(decoded)
    }
}

fn ext_for_mime(mime: &str) -> &'static str {
    match mime.split(';').next().unwrap_or("").trim() {
        "video/mp4" => ".mp4",
        "audio/mpeg" => ".mp3",
        "application/pdf" => ".pdf",
        "application/zip" => ".zip",
        "image/png" => ".png",
        "image/jpeg" => ".jpg",
        "text/plain" => ".txt",
        _ => ".bin",
    }
}

/// `HEAD` (falling back to a one-byte `GET`) learns size, range support and validators (§5.3).
pub async fn probe_url(client: &reqwest::Client, url: &str) -> Result<Probe, ErrorCode> {
    let head = client.head(url).send().await.ok();
    let (resp, ranged) = match head {
        Some(r) if r.status().is_success() => (r, false),
        _ => {
            let r = client
                .get(url)
                .header(RANGE, "bytes=0-0")
                .send()
                .await
                .map_err(|_| ErrorCode::NetUnreachable)?;
            (r, true)
        }
    };
    let status = resp.status().as_u16();
    if status == 404 || status == 410 {
        return Err(ErrorCode::NotFound);
    }
    if status == 401 || status == 403 {
        return Err(ErrorCode::AuthRequired);
    }
    if !resp.status().is_success() {
        return Err(ErrorCode::HttpError);
    }
    let final_url = resp.url().to_string();
    let headers = resp.headers().clone();
    let content_type = headers.get(CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let accepts_ranges = headers
        .get("accept-ranges")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("bytes"))
        .unwrap_or(false);
    let total = if ranged {
        headers
            .get("content-range")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.rsplit('/').next())
            .and_then(|t| t.parse::<u64>().ok())
    } else {
        headers.get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok())
    };
    let accepts_ranges = accepts_ranges || (ranged && status == 206);
    let validator = headers
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|v| format!("etag:{v}"))
        .or_else(|| headers.get(LAST_MODIFIED).and_then(|v| v.to_str().ok()).map(|v| format!("lm:{v}")));
    let filename = headers
        .get(CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .and_then(filename_from_disposition)
        .or_else(|| filename_from_url(&final_url))
        .map(|n| util::sanitize_filename(&n))
        .unwrap_or_else(|| {
            let mime = content_type.clone();
            format!("download{}", ext_for_mime(&mime))
        });
    Ok(Probe { total, accepts_ranges, validator, filename, content_type, final_url })
}

/* ---------- engine: public API ---------- */

fn dest_for(engine: &Engine, name: &str) -> PathBuf {
    let folder = match util::kind_from_name(name) {
        "video" => "Videos",
        "audio" => "Music",
        "image" => "Images",
        "document" => "Documents",
        "archive" => "Archives",
        "app" => "Apps",
        _ => "Other",
    };
    PathBuf::from(engine.settings().download_folder).join(folder)
}

fn validate_url(url: &str) -> Res<()> {
    if url.len() > 4096 || !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(err(ErrorCode::InvalidParams, "the link must be an http or https URL"));
    }
    Ok(())
}

/// `downloads.probe`: learns what a link is. Pages go through the extractor host.
pub async fn probe(engine: &Engine, url: &str) -> Res<Value> {
    validate_url(url)?;
    let p = probe_url(&engine.http, url).await.map_err(|c| err(c, probe_message(c)))?;
    if p.content_type.starts_with("text/html") {
        let r = crate::extractor::resolve(engine, url).await?;
        return Ok(json!({
            "kind": "media",
            "title": r.title,
            "duration": r.duration,
            "page": r.uploader,
            "formats": r.formats,
            "drm": false,
        }));
    }
    if p.content_type.contains("mpegurl") || p.content_type.contains("dash+xml") {
        return Err(err(ErrorCode::NotImplemented, "stream downloads (HLS/DASH) are not available in this build"));
    }
    Ok(json!({
        "kind": "direct",
        "filename": p.filename,
        "total": p.total.unwrap_or(0),
        "acceptsRanges": p.accepts_ranges,
        "drm": false,
        "formats": [],
    }))
}

fn probe_message(c: ErrorCode) -> &'static str {
    match c {
        ErrorCode::NotFound => "The server says this file is gone (404 or 410).",
        ErrorCode::AuthRequired => "The server needs a login for this link.",
        ErrorCode::NetUnreachable => "Could not reach the server.",
        _ => "The server did not give usable file details.",
    }
}

/// `downloads.add`: queues a job. Direct links and resolved media both run through the queue.
pub async fn add(engine: &Engine, params: &Value) -> Res<Value> {
    let url = params.get("url").and_then(Value::as_str).ok_or_else(|| err(ErrorCode::InvalidParams, "url is required"))?;
    validate_url(url)?;
    let kind = params.get("kind").and_then(Value::as_str).unwrap_or("direct");
    let name = params.get("name").and_then(Value::as_str).map(|n| util::sanitize_filename(n)).unwrap_or_default();
    let quality = params.get("options").and_then(|o| o.get("quality")).and_then(Value::as_str).map(str::to_string);
    if !["direct", "media"].contains(&kind) {
        return Err(err(ErrorCode::InvalidParams, "kind must be direct or media"));
    }
    let id = uuid::Uuid::new_v4().to_string();
    let now = now_s();
    let row = JobRow {
        id: id.clone(),
        url: url.to_string(),
        kind: kind.to_string(),
        state: "queued".into(),
        filename: name.clone(),
        destination: String::new(),
        total: 0,
        done: 0,
        priority: 1,
        retry_count: 0,
        group_id: None,
        created_at: now,
        updated_at: now,
        error: None,
        dest_dir: String::new(),
        validator: None,
        quality: quality.clone(),
        source_page: if kind == "media" { Some(url.to_string()) } else { None },
    };
    engine.db.job_insert(&row).map_err(internal)?;
    let job = Arc::new(Job::from_row(&row, vec![]));
    lock(&engine.jobs).insert(id.clone(), job.clone());
    engine.emit("download.state", json!({ "id": id, "state": "queued", "error": null }));
    pump_downloads(engine);
    Ok(json!({ "id": id, "state": "queued" }))
}

fn find_job(engine: &Engine, id: &str) -> Res<Arc<Job>> {
    lock(&engine.jobs).get(id).cloned().ok_or_else(|| err(ErrorCode::UnknownJob, format!("no download {id}")))
}

pub fn list(engine: &Engine) -> Value {
    let mut jobs: Vec<Arc<Job>> = lock(&engine.jobs).values().cloned().collect();
    jobs.sort_by(|a, b| b.priority.load(Ordering::SeqCst).cmp(&a.priority.load(Ordering::SeqCst)).then(a.created_ms.cmp(&b.created_ms)));
    json!({ "jobs": jobs.iter().map(|j| j.json()).collect::<Vec<_>>() })
}

pub fn pause(engine: &Engine, id: &str) -> Res<Value> {
    let job = find_job(engine, id)?;
    match job.state().as_str() {
        "downloading" | "probing" | "queued" => {
            job.ctrl.store(CTRL_PAUSE, Ordering::SeqCst);
            job.set_state("paused");
            persist(engine, &job, None);
            engine.emit("download.state", json!({ "id": id, "state": "paused", "error": null }));
        }
        _ => {}
    }
    Ok(json!({ "id": id, "state": job.state() }))
}

pub fn resume(engine: &Engine, id: &str) -> Res<Value> {
    let job = find_job(engine, id)?;
    match job.state().as_str() {
        "paused" | "failed" => {
            // A changed source (SRC_CHANGED) restarts from zero when the user asks to resume.
            if lock(&job.error).as_deref() == Some("SRC_CHANGED") {
                *lock(&job.validator) = None;
                for s in lock(&job.segs).iter() {
                    s.done.store(0, Ordering::SeqCst);
                }
                lock(&job.segs).clear();
                *lock(&job.resolved_url) = None;
                lock(&job.dest_dir).clear();
                let _ = std::fs::remove_file(part_path(engine, &job.id));
            }
            job.ctrl.store(CTRL_RUN, Ordering::SeqCst);
            job.set_state("queued");
            *lock(&job.error) = None;
            persist(engine, &job, None);
            engine.emit("download.state", json!({ "id": id, "state": "queued", "error": null }));
            pump_downloads(engine);
        }
        _ => {}
    }
    Ok(json!({ "id": id, "state": job.state() }))
}

pub fn cancel(engine: &Engine, id: &str) -> Res<Value> {
    let job = find_job(engine, id)?;
    job.ctrl.store(CTRL_CANCEL, Ordering::SeqCst);
    // A probing job has not written any bytes yet. Finishing it here releases its concurrency slot
    // even when its driver is already gone (a panicked task, or one that never got polled), so a
    // job cannot sit in `probing` and hold a slot the queue needs.
    if job.state() == "probing" || !job.running.load(Ordering::SeqCst) {
        job.running.store(false, Ordering::SeqCst);
        finish_cancel(engine, &job);
        pump_downloads(engine);
    }
    Ok(json!({ "cancelled": true }))
}

fn finish_cancel(engine: &Engine, job: &Arc<Job>) {
    if job.cancel_done.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = std::fs::remove_file(part_path(engine, &job.id));
    let _ = std::fs::remove_file(sidecar(&part_path(engine, &job.id)));
    let _ = engine.db.job_delete(&job.id);
    lock(&engine.jobs).remove(&job.id);
    job.set_state("cancelled");
    engine.emit("download.state", json!({ "id": job.id, "state": "cancelled", "error": null }));
}

/// `downloads.reorder {ids}`: first id gets the highest priority.
pub fn reorder(engine: &Engine, params: &Value) -> Res<Value> {
    let ids = params.get("ids").and_then(Value::as_array).ok_or_else(|| err(ErrorCode::InvalidParams, "ids is required"))?;
    let n = ids.len() as i64;
    for (i, id) in ids.iter().enumerate() {
        if let Some(id) = id.as_str() {
            if let Ok(job) = find_job(engine, id) {
                let p = n - i as i64;
                job.priority.store(p, Ordering::SeqCst);
                let _ = engine.db.job_set_priority(id, p);
            }
        }
    }
    Ok(json!({ "ok": true }))
}

pub fn set_priority(engine: &Engine, params: &Value) -> Res<Value> {
    let id = params.get("id").and_then(Value::as_str).ok_or_else(|| err(ErrorCode::InvalidParams, "id is required"))?;
    let p = params
        .get("priority")
        .and_then(Value::as_str)
        .and_then(priority_value)
        .ok_or_else(|| err(ErrorCode::InvalidParams, "priority must be high, normal or low"))?;
    let job = find_job(engine, id)?;
    job.priority.store(p, Ordering::SeqCst);
    engine.db.job_set_priority(id, p).map_err(internal)?;
    Ok(json!({ "id": id, "priority": p }))
}

/* ---------- scheduler ---------- */

fn part_path(engine: &Engine, id: &str) -> PathBuf {
    engine.cfg.data_dir.join("parts").join(format!("{id}.part"))
}

fn sidecar(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(".json");
    PathBuf::from(s)
}

fn persist(engine: &Engine, job: &Job, error: Option<&str>) {
    let now = now_s();
    let state = job.state();
    let _ = engine.db.job_set_state(&job.id, &state, error, now);
    let _ = engine.db.job_set_progress(&job.id, job.done() as i64, now);
    let segs: Vec<SegmentRow> = lock(&job.segs)
        .iter()
        .map(|s| SegmentRow { idx: s.idx, start: s.start as i64, end: s.end as i64, done: s.done.load(Ordering::SeqCst) as i64 })
        .collect();
    let _ = engine.db.segments_checkpoint(&job.id, &segs);
}

/// Starts queued jobs up to the concurrency limit, highest priority first, and emits progress.
pub fn pump_downloads(engine: &Engine) {
    let limit = engine.settings().downloads_at_once.max(1) as usize;
    let jobs: Vec<Arc<Job>> = lock(&engine.jobs).values().cloned().collect();
    let now = now_ms();
    let mut running = 0;
    for job in &jobs {
        if job.running.load(Ordering::SeqCst) {
            running += 1;
            let done = job.done();
            let prev = job.prev_done.swap(done, Ordering::SeqCst);
            let prev_t = job.prev_ms.swap(now, Ordering::SeqCst);
            let dt = (now - prev_t).max(1) as u64;
            let bps = done.saturating_sub(prev) * 1000 / dt;
            job.bps.store(bps, Ordering::SeqCst);
            let total = job.total.load(Ordering::SeqCst);
            let eta = if bps > 0 && total > done { Some((total - done) / bps) } else { None };
            engine.emit(
                "download.progress",
                json!({ "id": job.id, "done": done, "total": total, "bps": bps, "eta_s": eta }),
            );
        }
    }
    let mut queued: Vec<Arc<Job>> = jobs
        .into_iter()
        .filter(|j| j.state() == "queued" && !j.running.load(Ordering::SeqCst) && j.ctrl.load(Ordering::SeqCst) == CTRL_RUN)
        .collect();
    queued.sort_by(|a, b| b.priority.load(Ordering::SeqCst).cmp(&a.priority.load(Ordering::SeqCst)).then(a.created_ms.cmp(&b.created_ms)));
    for job in queued {
        if running >= limit {
            break;
        }
        running += 1;
        job.running.store(true, Ordering::SeqCst);
        job.set_state("probing");
        engine.emit("download.state", json!({ "id": job.id, "state": "probing", "error": null }));
        let eng = engine.clone();
        engine.spawn(async move {
            // The guard releases the slot on every exit path: normal completion, a driver that
            // panics mid-probe, and an aborted task. Without it a panicking probe left `running`
            // set forever, so the job sat in `probing` and kept a concurrency slot.
            let _slot = SlotGuard { engine: eng.clone(), job: job.clone() };
            run_job(eng, job).await;
        });
    }
}

/// Owns one job's concurrency slot while its driver task lives.
struct SlotGuard {
    engine: Engine,
    job: Arc<Job>,
}

impl Drop for SlotGuard {
    fn drop(&mut self) {
        self.job.running.store(false, Ordering::SeqCst);
        // A driver that died mid-flight (panic) never reported an outcome. Fail the job rather
        // than leave a card that says "checking the link" forever.
        let state = self.job.state();
        if state == "probing" || state == "downloading" {
            let code = ErrorCode::Internal.as_str();
            self.job.set_state("failed");
            *lock(&self.job.error) = Some(code.to_string());
            persist(&self.engine, &self.job, Some(code));
            self.engine.emit("download.state", json!({ "id": self.job.id, "state": "failed", "error": code }));
        }
        pump_downloads(&self.engine);
    }
}

/// Startup: jobs that were active before a restart are queued again (section 3.4 "Jobs in flight reload").
pub fn load_jobs(engine: &Engine) {
    let rows = match engine.db.jobs_all() {
        Ok(r) => r,
        Err(_) => return,
    };
    for row in rows {
        let segs = engine.db.segments_get(&row.id).unwrap_or_default();
        let job = Arc::new(Job::from_row(&row, segs));
        if job.state() == "queued" && row.state != "queued" {
            let _ = engine.db.job_set_state(&row.id, "queued", None, now_s());
        }
        lock(&engine.jobs).insert(row.id.clone(), job);
    }
}

/* ---------- the driver ---------- */

enum Outcome {
    Finished,
    Paused,
    Cancelled,
    Failed(ErrorCode),
    NoSpace,
}

async fn run_job(engine: Engine, job: Arc<Job>) {
    let outcome = drive(&engine, &job).await;
    match outcome {
        Outcome::Paused => {
            persist(&engine, &job, None);
            engine.emit("download.state", json!({ "id": job.id, "state": "paused", "error": null }));
        }
        Outcome::Cancelled => finish_cancel(&engine, &job),
        Outcome::NoSpace => {
            job.set_state("paused");
            *lock(&job.error) = Some("NO_SPACE".into());
            job.ctrl.store(CTRL_PAUSE, Ordering::SeqCst);
            persist(&engine, &job, Some("NO_SPACE"));
            engine.emit("download.state", json!({ "id": job.id, "state": "paused", "error": "NO_SPACE" }));
            engine.emit("notification.request", json!({ "kind": "no-space", "title": "Out of space", "body": "Free some space, then resume." }));
        }
        Outcome::Failed(code) => {
            job.set_state("failed");
            *lock(&job.error) = Some(code.as_str().to_string());
            persist(&engine, &job, Some(code.as_str()));
            engine.emit("download.state", json!({ "id": job.id, "state": "failed", "error": code.as_str() }));
            engine.emit(
                "notification.request",
                json!({ "kind": "download-failed", "title": "Download failed", "body": format!("{}: {}", lock(&job.name).clone(), code.as_str()) }),
            );
        }
        Outcome::Finished => complete(&engine, &job).await,
    }
}

/// Test hook: when `DROPGRAB_TEST_PANIC_PROBE` is set and matches part of a job's URL, the probe
/// panics on purpose so the regression test can prove the concurrency slot is still released.
/// Debug builds only, and inert unless the variable is set.
#[cfg(debug_assertions)]
fn maybe_panic_probe(url: &str) {
    if let Ok(want) = std::env::var("DROPGRAB_TEST_PANIC_PROBE") {
        if !want.is_empty() && url.contains(&want) {
            panic!("injected probe panic (DROPGRAB_TEST_PANIC_PROBE)");
        }
    }
}

#[cfg(not(debug_assertions))]
fn maybe_panic_probe(_url: &str) {}

async fn drive(engine: &Engine, job: &Arc<Job>) -> Outcome {
    let settings = engine.settings();
    // 1. Media jobs resolve first; the resolved direct URL is reused on resume.
    let mut url = lock(&job.url).clone();
    maybe_panic_probe(&url);
    if job.kind == "media" && lock(&job.resolved_url).is_none() {
        let quality = lock(&job.quality).clone().unwrap_or(settings.default_quality.clone());
        let page = job.source_page.clone().unwrap_or(url.clone());
        match crate::extractor::resolve(engine, &page).await {
            Ok(r) => {
                let f = crate::extractor::pick_format(&r.formats, &quality);
                match f.and_then(|f| f.get("url").and_then(Value::as_str).map(str::to_string)) {
                    Some(u) => {
                        *lock(&job.resolved_url) = Some(u.clone());
                        *lock(&job.name) = if lock(&job.name).is_empty() { r.title.clone() } else { lock(&job.name).clone() };
                        url = u;
                    }
                    None => return Outcome::Failed(ErrorCode::ExtractFailed),
                }
            }
            Err(e) => {
                let code = ErrorCode::from_str_code(&e.code).unwrap_or(ErrorCode::ExtractFailed);
                return Outcome::Failed(code);
            }
        }
    } else if let Some(u) = lock(&job.resolved_url).clone() {
        url = u;
    }
    if job.ctrl.load(Ordering::SeqCst) == CTRL_CANCEL {
        return Outcome::Cancelled;
    }

    // 2. Probe (size, range support, validator). Also re-checks the source on resume.
    let probe = match probe_url(&engine.http, &url).await {
        Ok(p) => p,
        Err(c) => return Outcome::Failed(c),
    };
    if job.ctrl.load(Ordering::SeqCst) == CTRL_PAUSE {
        return Outcome::Paused;
    }
    if job.ctrl.load(Ordering::SeqCst) == CTRL_CANCEL {
        return Outcome::Cancelled;
    }
    let known_validator = lock(&job.validator).clone();
    if let (Some(known), Some(now)) = (&known_validator, &probe.validator) {
        if known != now && job.done() > 0 {
            return Outcome::Failed(ErrorCode::SrcChanged);
        }
    }
    let name = {
        let mut n = lock(&job.name);
        if n.is_empty() {
            *n = probe.filename.clone();
        }
        n.clone()
    };
    let total = match probe.total {
        Some(t) => t,
        None => return Outcome::Failed(ErrorCode::HttpError),
    };
    job.total.store(total, Ordering::SeqCst);
    if lock(&job.dest_dir).is_empty() {
        *lock(&job.dest_dir) = dest_for(engine, &name).display().to_string();
    }
    *lock(&job.validator) = probe.validator.clone();
    job.range_ok.store(probe.accepts_ranges, Ordering::SeqCst);
    let _ = engine.db.job_set_meta(
        &job.id,
        total as i64,
        &name,
        &lock(&job.dest_dir).clone(),
        probe.validator.as_deref(),
        &url,
    );

    // 3. Segment plan. Without Range support the job is a single stream and cannot resume.
    if lock(&job.segs).is_empty() {
        let n = if probe.accepts_ranges { plan_segment_count(total, settings.segments_per_file) } else { 1 };
        let segs: Vec<Arc<Seg>> = split_ranges(total, n)
            .into_iter()
            .enumerate()
            .map(|(i, (s, e))| Arc::new(Seg { idx: i as i64, start: s, end: e, done: AtomicU64::new(0) }))
            .collect();
        *lock(&job.segs) = segs;
    }
    if !probe.accepts_ranges && job.done() > 0 {
        // Restart: the server cannot resume, so start over from one connection.
        for s in lock(&job.segs).iter() {
            s.done.store(0, Ordering::SeqCst);
        }
    }

    // 4. Part file (preallocated when the file system allows it).
    let part = part_path(engine, &job.id);
    if let Some(dir) = part.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Some(free) = crate::engine::free_bytes(part.parent().unwrap_or(Path::new("."))) {
        let have = job.total.load(Ordering::SeqCst).saturating_sub(job.done());
        if free < have + MIB {
            return Outcome::NoSpace;
        }
    }
    let p = part.clone();
    let prealloc = tokio::task::spawn_blocking(move || -> std::io::Result<std::fs::File> {
        let f = std::fs::OpenOptions::new().create(true).read(true).write(true).truncate(false).open(&p)?;
        if f.metadata()?.len() != total {
            f.set_len(total)?;
        }
        Ok(f)
    })
    .await;
    let file = match prealloc {
        Ok(Ok(f)) => Arc::new(f),
        Ok(Err(e)) if is_no_space(&e) => return Outcome::NoSpace,
        _ => return Outcome::Failed(ErrorCode::Permission),
    };
    if job.ctrl.load(Ordering::SeqCst) == CTRL_CANCEL {
        return Outcome::Cancelled;
    }

    // 5. Start: segments run in parallel; a checkpoint task writes progress every 2 s.
    job.set_state("downloading");
    persist(engine, job, None);
    engine.emit("download.state", json!({ "id": job.id, "state": "downloading", "error": null }));
    let active = Arc::new(AtomicBool::new(true));
    let checkpoint = {
        let eng = engine.clone();
        let job = job.clone();
        let active = active.clone();
        tokio::spawn(async move {
            let mut t = tokio::time::interval(Duration::from_secs(2));
            t.tick().await;
            while active.load(Ordering::SeqCst) {
                t.tick().await;
                persist(&eng, &job, None);
            }
        })
    };
    let segs = lock(&job.segs).clone();
    let futs = segs
        .iter()
        .filter(|s| s.start + s.done.load(Ordering::SeqCst) < s.end)
        .map(|s| segment_task(engine.clone(), job.clone(), s.clone(), url.clone(), file.clone()));
    let results = futures_util::future::join_all(futs).await;
    active.store(false, Ordering::SeqCst);
    checkpoint.abort();

    let mut outcome = Outcome::Finished;
    for r in results {
        match r {
            Ok(()) => {}
            Err(SegErr::Paused) => {
                if !matches!(outcome, Outcome::Cancelled) {
                    outcome = Outcome::Paused;
                }
            }
            Err(SegErr::Cancelled) => outcome = Outcome::Cancelled,
            Err(SegErr::NoSpace) => {
                if matches!(outcome, Outcome::Finished) {
                    outcome = Outcome::NoSpace;
                }
            }
            Err(SegErr::Fatal(c)) => {
                if matches!(outcome, Outcome::Finished) {
                    outcome = Outcome::Failed(c);
                }
            }
        }
    }
    if matches!(outcome, Outcome::Finished) && job.done() < total {
        outcome = Outcome::Failed(ErrorCode::NetUnreachable);
    }
    outcome
}

enum SegErr {
    Paused,
    Cancelled,
    NoSpace,
    Fatal(ErrorCode),
}

fn is_no_space(e: &std::io::Error) -> bool {
    let raw = e.raw_os_error();
    raw == Some(28) || raw == Some(112) || e.kind() == std::io::ErrorKind::StorageFull
}

async fn segment_task(engine: Engine, job: Arc<Job>, seg: Arc<Seg>, url: String, file: Arc<std::fs::File>) -> Result<(), SegErr> {
    let mut attempts: u32 = 0;
    let mut forbidden_retried = false;
    loop {
        match job.ctrl.load(Ordering::SeqCst) {
            CTRL_PAUSE => return Err(SegErr::Paused),
            CTRL_CANCEL => return Err(SegErr::Cancelled),
            _ => {}
        }
        let from = seg.start + seg.done.load(Ordering::SeqCst);
        if from >= seg.end {
            return Ok(());
        }
        let whole = seg.start == 0 && seg.end == job.total.load(Ordering::SeqCst) && !job.range_ok.load(Ordering::SeqCst);
        let mut req = engine.http.get(&url);
        if !whole {
            req = req.header(RANGE, format!("bytes={}-{}", from, seg.end - 1));
        }
        let resp = match req.send().await {
            Ok(r) => r,
            Err(_) => {
                attempts += 1;
                if attempts > engine.cfg.retry_max_attempts {
                    return Err(SegErr::Fatal(ErrorCode::Timeout));
                }
                retry_sleep(&engine, attempts, None).await;
                continue;
            }
        };
        let status = resp.status().as_u16();
        match status {
            200 | 206 => {}
            404 | 410 => return Err(SegErr::Fatal(ErrorCode::NotFound)),
            401 => return Err(SegErr::Fatal(ErrorCode::AuthRequired)),
            403 => {
                if forbidden_retried {
                    return Err(SegErr::Fatal(ErrorCode::AuthRequired));
                }
                forbidden_retried = true;
                continue;
            }
            429 | 503 => {
                let after = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok());
                attempts += 1;
                if attempts > engine.cfg.retry_max_attempts {
                    return Err(SegErr::Fatal(ErrorCode::Timeout));
                }
                retry_sleep(&engine, attempts, after).await;
                continue;
            }
            _ => {
                attempts += 1;
                if attempts > engine.cfg.retry_max_attempts {
                    return Err(SegErr::Fatal(ErrorCode::HttpError));
                }
                retry_sleep(&engine, attempts, None).await;
                continue;
            }
        }
        if status == 200 && !whole && from > 0 {
            // The server ignored Range; the segment cannot resume here.
            return Err(SegErr::Fatal(ErrorCode::RangeUnsupported));
        }
        let mut stream = resp.bytes_stream();
        let mut failed_mid = false;
        loop {
            match job.ctrl.load(Ordering::SeqCst) {
                CTRL_PAUSE => return Err(SegErr::Paused),
                CTRL_CANCEL => return Err(SegErr::Cancelled),
                _ => {}
            }
            let next = stream.next().await;
            let chunk = match next {
                None => break,
                Some(Ok(c)) => c,
                Some(Err(_)) => {
                    failed_mid = true;
                    break;
                }
            };
            let cur = seg.start + seg.done.load(Ordering::SeqCst);
            if cur >= seg.end {
                break;
            }
            let take = (seg.end - cur).min(chunk.len() as u64) as usize;
            let data = chunk.slice(0..take);
            let f = file.clone();
            let write = tokio::task::spawn_blocking(move || crate::transfer::write_at(&f, &data, cur)).await;
            match write {
                Ok(Ok(())) => {
                    seg.done.fetch_add(take as u64, Ordering::SeqCst);
                }
                Ok(Err(e)) if is_no_space(&e) => return Err(SegErr::NoSpace),
                _ => return Err(SegErr::Fatal(ErrorCode::Permission)),
            }
        }
        if failed_mid {
            attempts += 1;
            if attempts > engine.cfg.retry_max_attempts {
                return Err(SegErr::Fatal(ErrorCode::NetUnreachable));
            }
            retry_sleep(&engine, attempts, None).await;
            continue;
        }
        if seg.start + seg.done.load(Ordering::SeqCst) >= seg.end {
            return Ok(());
        }
        // Short body without error: retry the remainder.
        attempts += 1;
        if attempts > engine.cfg.retry_max_attempts {
            return Err(SegErr::Fatal(ErrorCode::NetUnreachable));
        }
        retry_sleep(&engine, attempts, None).await;
    }
}

/// Backoff: 1, 2, 4, 8, 16, 30 s (section 5.3). Honors Retry-After when given.
async fn retry_sleep(engine: &Engine, attempt: u32, retry_after: Option<u64>) {
    let base = engine.cfg.retry_base.as_millis() as u64;
    let exp = base.saturating_mul(1u64 << attempt.min(5));
    let mut wait = exp.min(30_000);
    if let Some(s) = retry_after {
        wait = wait.max(s * 1000).min(120_000);
    }
    tokio::time::sleep(Duration::from_millis(wait)).await;
}

/// Verifies, moves the part file to its folder, and adds the library item.
async fn complete(engine: &Engine, job: &Arc<Job>) {
    let part = part_path(engine, &job.id);
    let name = lock(&job.name).clone();
    let total = job.total.load(Ordering::SeqCst);
    let p = part.clone();
    let sha = tokio::task::spawn_blocking(move || crate::transfer::sha256_file(&p)).await;
    let sha = match sha {
        Ok(Ok(s)) => s,
        _ => return finish_failed(engine, job, ErrorCode::Permission),
    };
    let dest_dir = PathBuf::from(lock(&job.dest_dir).clone());
    if std::fs::create_dir_all(&dest_dir).is_err() {
        return finish_failed(engine, job, ErrorCode::Permission);
    }
    let final_path = util::unique_path(&dest_dir, &name);
    let (from, to) = (part.clone(), final_path.clone());
    let moved = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        match std::fs::rename(&from, &to) {
            Ok(()) => Ok(()),
            Err(_) => {
                // Different file system: copy then remove.
                std::fs::copy(&from, &to)?;
                std::fs::remove_file(&from)
            }
        }
    })
    .await;
    match moved {
        Ok(Ok(())) => {}
        Ok(Err(e)) if is_no_space(&e) => return finish_failed(engine, job, ErrorCode::NoSpace),
        _ => return finish_failed(engine, job, ErrorCode::Permission),
    }
    let _ = std::fs::remove_file(sidecar(&part));
    let source_url = job.source_page.clone().or_else(|| Some(lock(&job.url).clone()));
    let item = crate::db::ItemRow {
        id: uuid::Uuid::new_v4().to_string(),
        path: final_path.display().to_string(),
        display_name: final_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(name.clone()),
        mime: Some(util::mime_from_name(&name).to_string()),
        kind: util::kind_from_name(&name).to_string(),
        size: total as i64,
        sha256: Some(sha),
        origin: "download".into(),
        source_url: source_url.clone(),
        source_device: None,
        created_at: now_s(),
    };
    let item_id = item.id.clone();
    let _ = engine.db.item_insert(&item);
    job.set_state("finished");
    job.finished_ms.store(now_ms(), Ordering::SeqCst);
    *lock(&job.error) = None;
    persist(engine, job, None);
    let host = source_url.as_deref().and_then(util::host_of).unwrap_or_else(|| "a link".into());
    engine.emit("download.state", json!({ "id": job.id, "state": "finished", "error": null }));
    engine.emit("download.finished", json!({ "id": job.id, "name": name, "size": total, "itemId": item_id, "host": host }));
    engine.emit("library.changed", json!({ "reason": "download-finished", "ids": [item_id] }));
    engine.emit(
        "notification.request",
        json!({ "kind": "download-finished", "title": "Download finished", "body": format!("{} · {} MB", name, total / 1_000_000) }),
    );
}

fn finish_failed(engine: &Engine, job: &Arc<Job>, code: ErrorCode) {
    job.set_state("failed");
    *lock(&job.error) = Some(code.as_str().to_string());
    persist(engine, job, Some(code.as_str()));
    engine.emit("download.state", json!({ "id": job.id, "state": "failed", "error": code.as_str() }));
}

pub fn queue_len(engine: &Engine) -> usize {
    lock(&engine.jobs).values().filter(|j| !matches!(j.state().as_str(), "finished" | "cancelled")).count()
}

pub fn ensure(res: Result<(), crate::db::DbError>) -> Res<()> {
    res.map_err(internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_cover_total_without_gaps() {
        for total in [1u64, 7, 1000, 4 * MIB + 3, 900 * MIB] {
            for n in [1u64, 2, 4, 8, 16] {
                let r = split_ranges(total, n);
                assert_eq!(r.first().unwrap().0, 0);
                assert_eq!(r.last().unwrap().1, total);
                for w in r.windows(2) {
                    assert_eq!(w[0].1, w[1].0, "no gap or overlap");
                }
            }
        }
    }

    #[test]
    fn plan_respects_tiers_and_caps() {
        assert_eq!(plan_segment_count(500, 8), 1);
        assert_eq!(plan_segment_count(5 * MIB, 8), 4);
        assert_eq!(plan_segment_count(100 * MIB, 8), 8);
        assert_eq!(plan_segment_count(2048 * MIB, 16), 8, "per-host cap");
        assert_eq!(plan_segment_count(2048 * MIB, 2), 2, "settings cap");
    }

    #[test]
    fn disposition_and_url_names() {
        assert_eq!(filename_from_disposition("attachment; filename=\"a b.pdf\"").as_deref(), Some("a b.pdf"));
        assert_eq!(filename_from_disposition("attachment; filename*=UTF-8''r%C3%A9sum%C3%A9.pdf").as_deref(), Some("résumé.pdf"));
        assert_eq!(filename_from_url("https://x.test/a/b/file%20name.zip?x=1").as_deref(), Some("file name.zip"));
        assert_eq!(filename_from_url("https://x.test/"), None);
    }

    #[test]
    fn percent_decode_handles_edges() {
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("%zz"), "%zz");
        assert_eq!(percent_decode("%41%42"), "AB");
    }
}
