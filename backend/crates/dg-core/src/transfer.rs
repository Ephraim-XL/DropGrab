//! Transfer subsystem: session state machine (section 4.5), sender driver, and receiver-side
//! handlers for offer, upload ranges, status, complete and cancel (sections 4.4.x).
//!
//! Byte accounting: each file keeps `ranges`, the byte ranges confirmed on that side. On the
//! sender a range is confirmed when the receiver acknowledged the PUT (204). On the receiver a
//! range is confirmed when the bytes were written and synced. Progress is the covered length.

use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use dg_types::{
    ErrorCode, FileGrant, FileMeta, OfferRequest, OfferResponse, Sender, StatusResponse,
};
use serde_json::{json, Value};
use tokio::sync::{oneshot, Notify};

use crate::engine::{err, internal, lock, Engine, Res};
use crate::util::{self, now_ms, now_s};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dir {
    In,
    Out,
}

impl Dir {
    pub fn as_str(self) -> &'static str {
        match self {
            Dir::In => "in",
            Dir::Out => "out",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum St {
    Offered,
    Transferring,
    Paused,
    Done,
    Cancelled,
    Declined,
    Failed,
    TimedOut,
}

impl St {
    pub fn as_str(self) -> &'static str {
        match self {
            St::Offered => "offered",
            St::Transferring => "transferring",
            St::Paused => "paused",
            St::Done => "done",
            St::Cancelled => "cancelled",
            St::Declined => "declined",
            St::Failed => "failed",
            St::TimedOut => "timed-out",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, St::Done | St::Cancelled | St::Declined | St::Failed | St::TimedOut)
    }
}

pub const CTRL_RUN: u8 = 0;
pub const CTRL_PAUSE: u8 = 1;
pub const CTRL_CANCEL: u8 = 2;

pub struct FileEntry {
    pub fid: String,
    pub name: String,
    pub size: u64,
    pub mime: String,
    pub kind: String,
    pub sha256: Option<String>,
    pub token: String,
    /// Sender: the file on disk to read from.
    pub source: Option<PathBuf>,
    /// Receiver: the part file being written.
    pub part: Option<PathBuf>,
    /// Confirmed byte ranges, half-open `[start, end)`, sorted and merged.
    pub ranges: Vec<(u64, u64)>,
    pub state: &'static str,
    pub final_path: Option<PathBuf>,
    pub item_id: Option<String>,
}

pub struct Session {
    pub sid: String,
    pub dir: Dir,
    pub peer_id: String,
    pub peer_name: String,
    pub peer_fp: String,
    pub peer_addr: Option<(IpAddr, u16)>,
    pub state: std::sync::Mutex<St>,
    pub files: std::sync::Mutex<Vec<FileEntry>>,
    pub total: u64,
    pub control: AtomicU8,
    pub decision: std::sync::Mutex<Option<oneshot::Sender<bool>>>,
    pub wake: Notify,
    pub started_ms: i64,
    pub ended_ms: AtomicI64,
    pub error: std::sync::Mutex<Option<String>>,
    pub save_dir: PathBuf,
    pub verify_code: String,
    pub last_done: AtomicU64,
    pub last_tick_ms: AtomicI64,
    pub bps: AtomicU64,
    pub trust_on_accept: AtomicBool,
    pub trust_at_start: String,
    /// Staging ids this outgoing session sends from; removed when the session ends.
    pub staged: Vec<String>,
}

impl Session {
    pub fn state(&self) -> St {
        *lock(&self.state)
    }

    pub fn done_bytes(&self) -> u64 {
        lock(&self.files).iter().map(|f| covered(&f.ranges)).sum()
    }

    pub fn file_count(&self) -> usize {
        lock(&self.files).len()
    }

    pub fn all_done(&self) -> bool {
        lock(&self.files).iter().all(|f| f.state == "done")
    }

    pub fn control(&self) -> u8 {
        self.control.load(Ordering::SeqCst)
    }

    pub fn confirm_range(&self, idx: usize, start: u64, end: u64) {
        let mut files = lock(&self.files);
        if let Some(f) = files.get_mut(idx) {
            add_range(&mut f.ranges, start, end);
        }
    }
}

/* ---------- range math (property-tested in tests below) ---------- */

/// Inserts `[start, end)` into a sorted, merged range list.
pub fn add_range(ranges: &mut Vec<(u64, u64)>, start: u64, end: u64) {
    if start >= end {
        return;
    }
    ranges.push((start, end));
    ranges.sort_unstable();
    let mut merged: Vec<(u64, u64)> = Vec::with_capacity(ranges.len());
    for &(s, e) in ranges.iter() {
        match merged.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => merged.push((s, e)),
        }
    }
    *ranges = merged;
}

pub fn covered(ranges: &[(u64, u64)]) -> u64 {
    ranges.iter().map(|(s, e)| e - s).sum()
}

/// The first byte not yet confirmed (the resume offset).
pub fn contiguous_prefix(ranges: &[(u64, u64)]) -> u64 {
    match ranges.first() {
        Some(&(0, end)) => end,
        _ => 0,
    }
}

/// Missing sub-ranges of `[0, size)`.
pub fn missing(ranges: &[(u64, u64)], size: u64) -> Vec<(u64, u64)> {
    let mut out = Vec::new();
    let mut cursor = 0u64;
    for &(s, e) in ranges {
        if s > cursor {
            out.push((cursor, s.min(size)));
        }
        cursor = cursor.max(e);
    }
    if cursor < size {
        out.push((cursor, size));
    }
    out.retain(|(s, e)| s < e);
    out
}

/* ---------- file helpers ---------- */

pub fn write_at(file: &std::fs::File, buf: &[u8], offset: u64) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        file.write_all_at(buf, offset)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        let mut off = offset;
        let mut b = buf;
        while !b.is_empty() {
            let n = file.seek_write(b, off)?;
            b = &b[n..];
            off += n as u64;
        }
        Ok(())
    }
}

pub fn read_at(file: &std::fs::File, buf: &mut [u8], offset: u64) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        file.read_exact_at(buf, offset)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        let mut off = offset;
        let mut done = 0;
        while done < buf.len() {
            let n = file.seek_read(&mut buf[done..], off)?;
            if n == 0 {
                return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "short read"));
            }
            done += n;
            off += n as u64;
        }
        Ok(())
    }
}

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

fn part_dir(engine: &Engine) -> PathBuf {
    engine.cfg.data_dir.join("parts")
}

fn sidecar_path(part: &Path) -> PathBuf {
    let mut p = part.as_os_str().to_owned();
    p.push(".ranges.json");
    PathBuf::from(p)
}

fn persist_ranges(part: &Path, ranges: &[(u64, u64)]) {
    let list: Vec<[u64; 2]> = ranges.iter().map(|(s, e)| [*s, *e]).collect();
    if let Ok(json) = serde_json::to_vec(&list) {
        let tmp = sidecar_path(part).with_extension("tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, sidecar_path(part));
        }
    }
}

fn load_ranges(part: &Path) -> Vec<(u64, u64)> {
    std::fs::read(sidecar_path(part))
        .ok()
        .and_then(|b| serde_json::from_slice::<Vec<[u64; 2]>>(&b).ok())
        .map(|v| {
            let mut out = Vec::new();
            for [s, e] in v {
                add_range(&mut out, s, e);
            }
            out
        })
        .unwrap_or_default()
}

/* ---------- session bookkeeping ---------- */

pub fn sessions_json_row(s: &Session) -> Value {
    json!({
        "sid": s.sid,
        "direction": s.dir.as_str(),
        "peerId": s.peer_id,
        "peerName": s.peer_name,
        "state": s.state().as_str(),
        "total": s.total,
        "done": s.done_bytes(),
        "files": s.file_count(),
    })
}

fn persist_session(engine: &Engine, s: &Session) {
    let st = s.state();
    let ended = if st.is_terminal() { Some(now_s()) } else { None };
    let err_text = lock(&s.error).clone();
    let _ = engine.db.session_upsert(
        &s.sid,
        s.dir.as_str(),
        &s.peer_id,
        &s.peer_name,
        st.as_str(),
        s.total as i64,
        s.done_bytes() as i64,
        s.started_ms / 1000,
        ended,
        err_text.as_deref(),
    );
}

fn set_state(engine: &Engine, s: &Session, st: St) {
    *lock(&s.state) = st;
    persist_session(engine, s);
    engine.emit(
        "session.state",
        json!({ "sid": s.sid, "state": st.as_str(), "peerId": s.peer_id, "peerName": s.peer_name, "direction": s.dir.as_str() }),
    );
}

fn finish(engine: &Engine, s: &Session, st: St) {
    s.ended_ms.store(now_ms(), Ordering::SeqCst);
    set_state(engine, s, st);
    engine.emit(
        "session.finished",
        json!({ "sid": s.sid, "state": st.as_str(), "direction": s.dir.as_str(), "peerName": s.peer_name }),
    );
    // Staged uploads are one-shot: the bytes now live on the peer, so drop the local copy.
    crate::staging::release(engine, &s.staged);
}

/// Emits progress for every active session (called from the engine pump at 4 Hz).
pub fn pump_sessions(engine: &Engine) {
    let now = now_ms();
    let active: Vec<Arc<Session>> = lock(&engine.sessions).values().cloned().collect();
    for s in active {
        if s.state().is_terminal() {
            continue;
        }
        let done = s.done_bytes();
        let last = s.last_done.swap(done, Ordering::SeqCst);
        let last_t = s.last_tick_ms.swap(now, Ordering::SeqCst);
        let dt_ms = (now - last_t).max(1) as u64;
        let bps = done.saturating_sub(last) * 1000 / dt_ms;
        s.bps.store(bps, Ordering::SeqCst);
        if s.state() == St::Transferring {
            let eta = if bps > 0 { Some(s.total.saturating_sub(done) / bps) } else { None };
            engine.emit(
                "session.progress",
                json!({ "sid": s.sid, "done": done, "total": s.total, "bps": bps, "eta_s": eta }),
            );
        }
    }
}

fn find_session(engine: &Engine, sid: &str) -> Res<Arc<Session>> {
    lock(&engine.sessions)
        .get(sid)
        .cloned()
        .ok_or_else(|| err(ErrorCode::UnknownSession, format!("no session {sid}")))
}

/// Session id that the active or most recent session uses (for transfer screen commands).
pub fn active_session(engine: &Engine, sid: Option<&str>) -> Res<Arc<Session>> {
    if let Some(sid) = sid {
        return find_session(engine, sid);
    }
    let map = lock(&engine.sessions);
    let mut live: Vec<&Arc<Session>> = map.values().filter(|s| !s.state().is_terminal()).collect();
    live.sort_by_key(|s| std::cmp::Reverse(s.started_ms));
    live.first()
        .map(|s| (*s).clone())
        .ok_or_else(|| err(ErrorCode::UnknownSession, "no active session"))
}

/* ---------- receiver side ---------- */

pub fn executable_name(name: &str) -> bool {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    matches!(ext.as_str(), "exe" | "msi" | "apk" | "ipa" | "dmg" | "pkg" | "deb" | "rpm" | "appimage" | "bat" | "sh" | "scr")
}

/// Fingerprint of the peer a session belongs to, for checking the presented client certificate.
pub fn session_peer_fp(engine: &Engine, sid: &str) -> Option<String> {
    lock(&engine.sessions).get(sid).map(|s| s.peer_fp.clone())
}

/// `POST /api/v1/session/offer`. The request is held until the user decides or the offer
/// times out (section 4.4.3). Returns the decision in the response body.
pub async fn handle_offer(engine: Engine, req: OfferRequest, ip: IpAddr, port_hint: u16) -> Res<OfferResponse> {
    let _ = port_hint;
    let sender = req.sender.clone();
    if sender.device_id == engine.identity.device_id {
        return Err(err(ErrorCode::Forbidden, "offer from this device"));
    }
    if engine.is_blocked(&sender.device_id, &sender.fingerprint) {
        return Ok(decline("declined"));
    }
    // Sender identity must match a fingerprint we already know for that device (if any).
    if let Ok(Some(row)) = engine.db.device_get(&sender.device_id) {
        if !row.fingerprint.is_empty() && row.fingerprint != sender.fingerprint {
            return Err(err(ErrorCode::TlsMismatch, "sender fingerprint differs from the stored one"));
        }
    }
    if req.files.is_empty() || req.files.len() > 1000 {
        return Err(err(ErrorCode::InvalidParams, "offer must list 1 to 1000 files"));
    }
    let total: u64 = req.files.iter().map(|f| f.size).sum();
    if total != req.total_size {
        return Err(err(ErrorCode::InvalidParams, "totalSize does not match the file list"));
    }
    let sid = req
        .sid
        .clone()
        .filter(|s| is_valid_sid(s))
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    // The sender uses the sid from the response; it is kept on the receiver session too.
    if lock(&engine.sessions).contains_key(&sid) {
        return Err(err(ErrorCode::Busy, "session id already in use"));
    }
    let settings = engine.settings();
    let save_dir = engine.cfg.downloads_dir.join(&settings.save_location);
    if let Some(free) = crate::engine::free_bytes(&engine.cfg.downloads_dir) {
        if free < total + 1024 * 1024 {
            return Ok(OfferResponse {
                accepted: false,
                sid: None,
                files: None,
                chunk_size: None,
                streams: None,
                reason: Some(format!("no-space: need {total} bytes")),
            });
        }
    }
    let trust = trust_state(&engine, &sender);
    let auto = engine.db.device_get(&sender.device_id).ok().flatten().map(|r| r.auto_accept).unwrap_or(false);
    let auto_accept = (trust == "trusted" && auto) || (trust == "trusted" && !settings.ask_before_accepting);

    let files: Vec<FileEntry> = req
        .files
        .iter()
        .map(|f| FileEntry {
            fid: f.fid.clone(),
            name: util::sanitize_filename(&f.name),
            size: f.size,
            mime: f.mime.clone(),
            kind: util::kind_from_name(&f.name).to_string(),
            sha256: f.sha256.clone(),
            token: util::random_hex(16),
            source: None,
            part: None,
            ranges: Vec::new(),
            state: "pending",
            final_path: None,
            item_id: None,
        })
        .collect();
    let (tx, rx) = oneshot::channel::<bool>();
    let session = Arc::new(Session {
        sid: sid.clone(),
        dir: Dir::In,
        peer_id: sender.device_id.clone(),
        peer_name: sender.alias.clone(),
        peer_fp: sender.fingerprint.clone(),
        peer_addr: Some((ip, port_hint)),
        state: std::sync::Mutex::new(St::Offered),
        files: std::sync::Mutex::new(files),
        total,
        control: AtomicU8::new(CTRL_RUN),
        decision: std::sync::Mutex::new(Some(tx)),
        wake: Notify::new(),
        started_ms: now_ms(),
        ended_ms: AtomicI64::new(0),
        error: std::sync::Mutex::new(None),
        save_dir: save_dir.clone(),
        verify_code: crate::identity::verify_code(&engine.identity.fingerprint, &sender.fingerprint),
        last_done: AtomicU64::new(0),
        last_tick_ms: AtomicI64::new(now_ms()),
        bps: AtomicU64::new(0),
        trust_on_accept: AtomicBool::new(false),
        trust_at_start: trust.clone(),
        staged: Vec::new(),
    });
    lock(&engine.sessions).insert(sid.clone(), session.clone());
    persist_session(&engine, &session);

    if auto_accept {
        accept_session(&engine, &session, false).await?;
    } else {
        let files_json: Vec<Value> = lock(&session.files)
            .iter()
            .map(|f| json!({ "id": f.fid, "fid": f.fid, "name": f.name, "size": f.size, "kind": f.kind, "mime": f.mime }))
            .collect();
        let executable = lock(&session.files).iter().any(|f| executable_name(&f.name));
        engine.emit(
            "session.offered",
            json!({
                "sid": sid,
                "direction": "in",
                "peerId": sender.device_id,
                "peerName": sender.alias,
                "trust": trust,
                "verifyCode": session.verify_code,
                "files": files_json,
                "total": total,
                "savingTo": settings.save_location,
                "executable": executable,
                "offeredAt": now_ms(),
            }),
        );
        engine.emit(
            "notification.request",
            json!({
                "kind": "incoming",
                "title": format!("{} wants to send you {} files", sender.alias, req.files.len()),
                "body": format!("{} MB · Accept or decline", total / 1_000_000),
            }),
        );
    }

    let decided = tokio::time::timeout(engine.cfg.offer_timeout, rx).await;
    match decided {
        Ok(Ok(true)) => {
            // accept_session already ran for the local decision path; wait for it to build grants.
            let grants = build_grants(&session);
            Ok(OfferResponse {
                accepted: true,
                sid: Some(sid.clone()),
                files: Some(grants),
                chunk_size: Some(engine.cfg.chunk_size),
                streams: Some(engine.cfg.streams),
                reason: None,
            })
        }
        Ok(Ok(false)) => {
            finish(&engine, &session, St::Declined);
            lock(&engine.sessions).remove(&sid);
            Ok(decline("declined"))
        }
        _ => {
            if session.state() == St::Offered {
                finish(&engine, &session, St::TimedOut);
            }
            lock(&engine.sessions).remove(&sid);
            Ok(decline("timeout"))
        }
    }
}

fn is_valid_sid(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn decline(reason: &str) -> OfferResponse {
    OfferResponse { accepted: false, sid: None, files: None, chunk_size: None, streams: None, reason: Some(reason.into()) }
}

fn trust_state(engine: &Engine, sender: &Sender) -> String {
    match engine.db.device_get(&sender.device_id) {
        Ok(Some(row)) if row.trust == "trusted" && row.fingerprint == sender.fingerprint => "trusted".into(),
        Ok(Some(_)) => "seen".into(),
        _ => "unknown".into(),
    }
}

fn build_grants(session: &Session) -> std::collections::BTreeMap<String, FileGrant> {
    lock(&session.files)
        .iter()
        .map(|f| {
            (
                f.fid.clone(),
                FileGrant { token: f.token.clone(), resume_from: contiguous_prefix(&f.ranges) },
            )
        })
        .collect()
}

/// Accepts an offered session: prepares part files and tells the waiting offer handler.
/// `trust` marks the sender as trusted (section 6, "Accept and always trust this device").
pub async fn accept_session(engine: &Engine, session: &Arc<Session>, trust: bool) -> Res<()> {
    if session.state() != St::Offered {
        return Err(err(ErrorCode::Busy, "session is not waiting for a decision"));
    }
    tokio::fs::create_dir_all(&session.save_dir).await.map_err(internal)?;
    let pdir = part_dir(engine);
    tokio::fs::create_dir_all(&pdir).await.map_err(internal)?;
    // Collect the work first so no mutex guard is held across an await point.
    let plan: Vec<(String, u64, PathBuf)> = lock(&session.files)
        .iter()
        .map(|f| (f.fid.clone(), f.size, pdir.join(format!("{}-{}.part", session.sid, f.fid))))
        .collect();
    let prepared = tokio::task::spawn_blocking(move || -> std::io::Result<Vec<(String, PathBuf, Vec<(u64, u64)>)>> {
        let mut out = Vec::with_capacity(plan.len());
        for (fid, size, part) in plan {
            let ranges = if part.exists() {
                load_ranges(&part)
            } else {
                let file = std::fs::OpenOptions::new().create(true).write(true).truncate(false).open(&part)?;
                file.set_len(size)?;
                Vec::new()
            };
            out.push((fid, part, ranges));
        }
        Ok(out)
    })
    .await
    .map_err(internal)?
    .map_err(internal)?;
    {
        let mut files = lock(&session.files);
        for (f, (_, part, ranges)) in files.iter_mut().zip(prepared) {
            f.ranges = ranges;
            f.part = Some(part);
            f.state = "sending";
        }
    }
    if trust {
        session.trust_on_accept.store(true, Ordering::SeqCst);
        engine.upsert_device(&session.peer_id, &session.peer_name, "phone", "", &session.peer_fp).map_err(|e| e)?;
        engine.db.device_set_trust(&session.peer_id, "trusted", None).map_err(internal)?;
        engine.emit("device.changed", json!({ "deviceId": session.peer_id, "trust": "trusted" }));
    }
    set_state(engine, session, St::Transferring);
    let tx = lock(&session.decision).take();
    if let Some(tx) = tx {
        let _ = tx.send(true);
    }
    Ok(())
}

/// `session.respond` from the UI.
pub async fn respond(engine: &Engine, sid: &str, accept: bool, trust: bool) -> Res<Value> {
    let session = find_session(engine, sid)?;
    if session.dir != Dir::In || session.state() != St::Offered {
        return Err(err(ErrorCode::UnknownSession, "session is not waiting for a decision"));
    }
    if accept {
        accept_session(engine, &session, trust).await?;
        Ok(json!({ "accepted": true }))
    } else {
        let tx = lock(&session.decision).take();
        if let Some(tx) = tx {
            let _ = tx.send(false);
        }
        Ok(json!({ "accepted": false }))
    }
}

/// `PUT /api/v1/session/{sid}/file/{fid}`: writes one range, flushes, then acknowledges.
pub async fn receive_range(engine: Engine, sid: &str, fid: &str, token: &str, start: u64, end_incl: u64, total: u64, body: bytes::Bytes) -> Result<(), (u16, String)> {
    let session = lock(&engine.sessions).get(sid).cloned().ok_or((404, "unknown session".into()))?;
    match session.state() {
        St::Transferring => {}
        St::Paused => return Err((503, "paused".into())),
        _ => return Err((409, "session is not transferring".into())),
    }
    let (idx, part, size) = {
        let files = lock(&session.files);
        let idx = files.iter().position(|f| f.fid == fid).ok_or((404, "unknown file".into()))?;
        let f = &files[idx];
        if f.token != token {
            return Err((403, "bad token".into()));
        }
        (idx, f.part.clone().ok_or((409, "file not accepted".into()))?, f.size)
    };
    if total != size || end_incl < start || end_incl >= size || end_incl - start + 1 != body.len() as u64 {
        return Err((400, "bad Content-Range".into()));
    }
    let (s, e) = (start, end_incl + 1);
    let part2 = part.clone();
    let body2 = body.clone();
    tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        let file = std::fs::OpenOptions::new().write(true).open(&part2)?;
        write_at(&file, &body2, s)?;
        file.sync_data()?;
        Ok(())
    })
    .await
    .map_err(|e| (500, e.to_string()))?
    .map_err(|e| (507, format!("write failed: {e}")))?;
    session.confirm_range(idx, s, e);
    {
        let files = lock(&session.files);
        let ranges = files[idx].ranges.clone();
        persist_ranges(&part, &ranges);
    }
    Ok(())
}

pub fn status_for(engine: &Engine, sid: &str, fid: &str, token: &str) -> Result<StatusResponse, (u16, String)> {
    let session = lock(&engine.sessions).get(sid).cloned().ok_or((404, "unknown session".into()))?;
    let files = lock(&session.files);
    let f = files.iter().find(|f| f.fid == fid).ok_or((404, "unknown file".into()))?;
    if f.token != token {
        return Err((403, "bad token".into()));
    }
    Ok(StatusResponse { ranges: f.ranges.iter().map(|(s, e)| [*s, *e]).collect(), size: f.size })
}

/// `POST .../complete`: verifies the file, moves it into place, and adds it to the library.
pub async fn complete_file(engine: Engine, sid: &str, fid: &str, token: &str, sha_claim: Option<String>) -> Result<Value, (u16, String)> {
    let session = lock(&engine.sessions).get(sid).cloned().ok_or((404, "unknown session".into()))?;
    let (idx, part, name, size, expected, ranges) = {
        let files = lock(&session.files);
        let idx = files.iter().position(|f| f.fid == fid).ok_or((404, "unknown file".into()))?;
        let f = &files[idx];
        if f.token != token {
            return Err((403, "bad token".into()));
        }
        (idx, f.part.clone().ok_or((409, "file not accepted".into()))?, f.name.clone(), f.size, f.sha256.clone(), f.ranges.clone())
    };
    if covered(&ranges) != size {
        return Err((409, "file is incomplete".into()));
    }
    let part2 = part.clone();
    let sha = tokio::task::spawn_blocking(move || sha256_file(&part2))
        .await
        .map_err(|e| (500, e.to_string()))?
        .map_err(|e| (500, e.to_string()))?;
    if let Some(claim) = sha_claim.or(expected.clone()) {
        if claim != sha {
            // Section 4.4.6: on mismatch the sender retries; the receiver drops the claimed ranges.
            let mut files = lock(&session.files);
            files[idx].ranges.clear();
            persist_ranges(&part, &[]);
            return Err((422, "hash mismatch".into()));
        }
    }
    let final_path = util::unique_path(&session.save_dir, &name);
    let fp = final_path.clone();
    let p2 = part.clone();
    tokio::task::spawn_blocking(move || std::fs::rename(&p2, &fp))
        .await
        .map_err(|e| (500, e.to_string()))?
        .map_err(|e| (500, e.to_string()))?;
    let _ = std::fs::remove_file(sidecar_path(&part));
    let item_id = uuid::Uuid::new_v4().to_string();
    let item = crate::db::ItemRow {
        id: item_id.clone(),
        path: final_path.display().to_string(),
        display_name: final_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(name.clone()),
        mime: Some(util::mime_from_name(&name).to_string()),
        kind: util::kind_from_name(&name).to_string(),
        size: size as i64,
        sha256: Some(sha.clone()),
        origin: "received".into(),
        source_url: None,
        source_device: Some(session.peer_id.clone()),
        created_at: now_s(),
    };
    engine.db.item_insert(&item).map_err(|e| (500, e.to_string()))?;
    {
        let mut files = lock(&session.files);
        files[idx].state = "done";
        files[idx].final_path = Some(final_path.clone());
        files[idx].item_id = Some(item_id.clone());
    }
    engine.emit("library.changed", json!({ "reason": "received", "ids": [item_id] }));
    if session.all_done() {
        finish(&engine, &session, St::Done);
        let secs = (now_ms() - session.started_ms) / 1000;
        *lock(&session.error) = None;
        engine.emit(
            "notification.request",
            json!({ "kind": "received-finished", "title": "Received from ".to_string() + &session.peer_name,
                    "body": format!("{} files · {} s", session.file_count(), secs) }),
        );
    } else {
        persist_session(&engine, &session);
    }
    Ok(json!({ "verified": true, "sha256": sha }))
}

pub fn cancel_incoming(engine: &Engine, session: &Arc<Session>) {
    session.control.store(CTRL_CANCEL, Ordering::SeqCst);
    let parts: Vec<PathBuf> = lock(&session.files).iter().filter_map(|f| f.part.clone()).collect();
    for p in parts {
        let _ = std::fs::remove_file(&p);
        let _ = std::fs::remove_file(sidecar_path(&p));
    }
    let tx = lock(&session.decision).take();
    if let Some(tx) = tx {
        let _ = tx.send(false);
    }
    finish(engine, session, St::Cancelled);
}

/* ---------- sender side ---------- */

/// `session.send`: builds the offer and starts the sender driver. Returns the session id.
pub async fn start_send(engine: Engine, peer_id: &str, items: Vec<Value>) -> Res<String> {
    let peer = engine
        .peer_for(peer_id)
        .ok_or_else(|| err(ErrorCode::NetUnreachable, "device is not reachable on this network"))?;
    if items.is_empty() {
        return Err(err(ErrorCode::InvalidParams, "no items to send"));
    }
    let mut entries = Vec::new();
    let mut staged_ids: Vec<String> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let (path, name) = resolve_item(&engine, item)?;
        if let Some(id) = item.get("stagedId").and_then(Value::as_str) {
            if !staged_ids.iter().any(|s| s == id) {
                staged_ids.push(id.to_string());
            }
        }
        let meta = tokio::fs::metadata(&path).await.map_err(|_| err(ErrorCode::InvalidParams, format!("cannot read {}", path.display())))?;
        if !meta.is_file() {
            return Err(err(ErrorCode::InvalidParams, "only regular files can be sent"));
        }
        entries.push((format!("f{}", i + 1), path, name, meta.len()));
    }
    let total: u64 = entries.iter().map(|e| e.3).sum();
    let sid = uuid::Uuid::new_v4().to_string();
    let mut files_meta = Vec::new();
    let mut file_entries = Vec::new();
    for (fid, path, name, size) in &entries {
        let p = path.clone();
        let sha = tokio::task::spawn_blocking(move || sha256_file(&p)).await.map_err(internal)?.map_err(internal)?;
        files_meta.push(FileMeta {
            fid: fid.clone(),
            name: name.clone(),
            size: *size,
            mime: util::mime_from_name(name).to_string(),
            sha256: Some(sha.clone()),
            modified: None,
        });
        file_entries.push(FileEntry {
            fid: fid.clone(),
            name: name.clone(),
            size: *size,
            mime: util::mime_from_name(name).to_string(),
            kind: util::kind_from_name(name).to_string(),
            sha256: Some(sha),
            token: String::new(),
            source: Some(path.clone()),
            part: None,
            ranges: Vec::new(),
            state: "pending",
            final_path: None,
            item_id: None,
        });
    }
    let session = Arc::new(Session {
        sid: sid.clone(),
        dir: Dir::Out,
        peer_id: peer.device_id.clone(),
        peer_name: peer.alias.clone(),
        peer_fp: peer.fingerprint.clone(),
        peer_addr: Some((peer.ip, peer.port)),
        state: std::sync::Mutex::new(St::Offered),
        files: std::sync::Mutex::new(file_entries),
        total,
        control: AtomicU8::new(CTRL_RUN),
        decision: std::sync::Mutex::new(None),
        wake: Notify::new(),
        started_ms: now_ms(),
        ended_ms: AtomicI64::new(0),
        error: std::sync::Mutex::new(None),
        save_dir: PathBuf::new(),
        verify_code: crate::identity::verify_code(&engine.identity.fingerprint, &peer.fingerprint),
        last_done: AtomicU64::new(0),
        last_tick_ms: AtomicI64::new(now_ms()),
        bps: AtomicU64::new(0),
        trust_on_accept: AtomicBool::new(false),
        trust_at_start: trust_state(&engine, &dg_types::Sender {
            device_id: peer.device_id.clone(),
            alias: peer.alias.clone(),
            device_type: peer.device_type.clone(),
            fingerprint: peer.fingerprint.clone(),
        }),
        staged: staged_ids,
    });
    lock(&engine.sessions).insert(sid.clone(), session.clone());
    set_state(&engine, &session, St::Offered);
    let offer = OfferRequest { sid: Some(sid.clone()), sender: engine.self_sender(), files: files_meta, total_size: total };
    let eng = engine.clone();
    engine.spawn(async move {
        run_outgoing(eng, session, offer).await;
    });
    Ok(sid)
}

fn resolve_item(engine: &Engine, item: &Value) -> Res<(PathBuf, String)> {
    // A file staged through `files.stage` (browser upload) may be sent by id.
    if let Some(staged_id) = item.get("stagedId").and_then(Value::as_str) {
        let st = crate::staging::get(engine, staged_id)
            .ok_or_else(|| err(ErrorCode::NotFound, format!("no staged file {staged_id}")))?;
        return Ok((st.path, st.name));
    }
    if let Some(p) = item.get("path").and_then(Value::as_str) {
        let path = PathBuf::from(p);
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| path.file_name().map(|n| n.to_string_lossy().to_string()))
            .ok_or_else(|| err(ErrorCode::InvalidParams, "item has no name"))?;
        return Ok((path, util::sanitize_filename(&name)));
    }
    if let Some(id) = item.get("id").and_then(Value::as_str) {
        let row = engine
            .db
            .item_get(id)
            .map_err(internal)?
            .ok_or_else(|| err(ErrorCode::UnknownItem, format!("no library item {id}")))?;
        return Ok((PathBuf::from(&row.path), util::sanitize_filename(&row.display_name)));
    }
    Err(err(ErrorCode::InvalidParams, "item needs a path or a library id"))
}

fn peer_url(addr: (IpAddr, u16), path: &str) -> String {
    match addr.0 {
        IpAddr::V6(v6) => format!("https://[{v6}]:{}{path}", addr.1),
        IpAddr::V4(v4) => format!("https://{v4}:{}{path}", addr.1),
    }
}

/// Sender driver: offer, upload missing ranges with bounded parallelism, complete each file.
async fn run_outgoing(engine: Engine, session: Arc<Session>, offer: OfferRequest) {
    let addr = match session.peer_addr {
        Some(a) => a,
        None => return fail(&engine, &session, ErrorCode::NetUnreachable),
    };
    let client = match crate::identity::mtls_client(&engine.identity, &session.peer_fp, Duration::from_secs(120)) {
        Ok(c) => c,
        Err(_) => return fail(&engine, &session, ErrorCode::Internal),
    };
    let resp = match client.post(peer_url(addr, "/api/v1/session/offer")).json(&offer).timeout(engine.cfg.offer_timeout + Duration::from_secs(10)).send().await {
        Ok(r) => r,
        Err(e) => {
            let code = if e.to_string().contains("TLS_MISMATCH") { ErrorCode::TlsMismatch } else { ErrorCode::NetUnreachable };
            return fail(&engine, &session, code);
        }
    };
    if !resp.status().is_success() {
        return fail(&engine, &session, ErrorCode::Forbidden);
    }
    let answer: OfferResponse = match resp.json().await {
        Ok(a) => a,
        Err(_) => return fail(&engine, &session, ErrorCode::Internal),
    };
    if !answer.accepted {
        let reason = answer.reason.unwrap_or_default();
        let st = if reason == "timeout" { St::TimedOut } else { St::Declined };
        return finish(&engine, &session, st);
    }
    let sid = match answer.sid.clone() {
        Some(s) if is_valid_sid(&s) => s,
        _ => return fail(&engine, &session, ErrorCode::Internal),
    };
    let grants = answer.files.unwrap_or_default();
    let chunk = answer.chunk_size.unwrap_or(engine.cfg.chunk_size).max(64 * 1024);
    let streams = answer.streams.unwrap_or(engine.cfg.streams).clamp(1, 8) as usize;
    {
        let mut files = lock(&session.files);
        for f in files.iter_mut() {
            if let Some(g) = grants.get(&f.fid) {
                f.token = g.token.clone();
            }
        }
    }
    set_state(&engine, &session, St::Transferring);

    let file_count = session.file_count();
    for idx in 0..file_count {
        if session.control() == CTRL_CANCEL {
            break;
        }
        // Resume: ask the receiver what it already has.
        let (fid, token, size) = {
            let files = lock(&session.files);
            (files[idx].fid.clone(), files[idx].token.clone(), files[idx].size)
        };
        let status_url = peer_url(addr, &format!("/api/v1/session/{sid}/file/{fid}/status?token={token}"));
        match client.get(&status_url).send().await {
            Ok(r) if r.status().is_success() => {
                if let Ok(st) = r.json::<StatusResponse>().await {
                    let mut files = lock(&session.files);
                    files[idx].ranges.clear();
                    for [s, e] in st.ranges {
                        add_range(&mut files[idx].ranges, s, e.min(size));
                    }
                }
            }
            _ => return fail(&engine, &session, ErrorCode::NetUnreachable),
        }
        let todo = missing(&lock(&session.files)[idx].ranges, size);
        let mut chunks = Vec::new();
        for (s, e) in todo {
            let mut cur = s;
            while cur < e {
                let end = (cur + chunk).min(e);
                chunks.push((cur, end));
                cur = end;
            }
        }
        {
            let mut files = lock(&session.files);
            files[idx].state = "sending";
        }
        let results: Vec<Result<(), ErrorCode>> = {
            use futures_util::StreamExt;
            let eng = &engine;
            let sess = &session;
            let cli = &client;
            futures_util::stream::iter(chunks)
                .map(|(s, e)| async move { upload_chunk(eng, sess, cli, addr, idx, s, e).await })
                .buffer_unordered(streams)
                .collect()
                .await
        };
        if let Some(code) = results.iter().find_map(|r| r.as_ref().err().copied()) {
            if session.control() == CTRL_CANCEL {
                break;
            }
            return fail(&engine, &session, code);
        }
        // Complete with the file's hash.
        let sha = lock(&session.files)[idx].sha256.clone().unwrap_or_default();
        let url = peer_url(addr, &format!("/api/v1/session/{sid}/file/{fid}/complete?token={token}"));
        let done = client.post(url).json(&dg_types::CompleteRequest { sha256: sha }).send().await;
        match done {
            Ok(r) if r.status().is_success() => {
                lock(&session.files)[idx].state = "done";
            }
            Ok(r) if r.status().as_u16() == 422 => return fail(&engine, &session, ErrorCode::HashMismatch),
            _ => return fail(&engine, &session, ErrorCode::NetUnreachable),
        }
    }

    if session.control() == CTRL_CANCEL {
        let url = peer_url(addr, &format!("/api/v1/session/{sid}/cancel"));
        let _ = client.post(url).send().await;
        return finish(&engine, &session, St::Cancelled);
    }
    if session.all_done() {
        finish(&engine, &session, St::Done);
    }
}

fn fail(engine: &Engine, session: &Session, code: ErrorCode) {
    *lock(&session.error) = Some(code.as_str().to_string());
    finish(engine, session, St::Failed);
}

/// Uploads one range with retries, honoring pause and cancel (section 4.4.5, 4.10).
async fn upload_chunk(
    engine: &Engine,
    session: &Arc<Session>,
    client: &reqwest::Client,
    addr: (IpAddr, u16),
    idx: usize,
    start: u64,
    end: u64,
) -> Result<(), ErrorCode> {
    let (sid, fid, token, source) = {
        let files = lock(&session.files);
        let f = &files[idx];
        (session.sid.clone(), f.fid.clone(), f.token.clone(), f.source.clone().ok_or(ErrorCode::Internal)?)
    };
    let mut attempts = 0u32;
    loop {
        while session.control() == CTRL_PAUSE {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        if session.control() == CTRL_CANCEL {
            return Err(ErrorCode::Internal);
        }
        let src = source.clone();
        let len = (end - start) as usize;
        let data = tokio::task::spawn_blocking(move || -> std::io::Result<Vec<u8>> {
            let f = std::fs::File::open(&src)?;
            let mut buf = vec![0u8; len];
            read_at(&f, &mut buf, start)?;
            Ok(buf)
        })
        .await
        .map_err(|_| ErrorCode::Internal)?
        .map_err(|_| ErrorCode::SrcChanged)?;
        let size = lock(&session.files)[idx].size;
        let url = peer_url(addr, &format!("/api/v1/session/{sid}/file/{fid}?token={token}"));
        let res = client
            .put(url)
            .header("Content-Range", format!("bytes {}-{}/{}", start, end - 1, size))
            .header("Content-Type", "application/octet-stream")
            .body(data)
            .send()
            .await;
        match res {
            Ok(r) if r.status().as_u16() == 204 => {
                session.confirm_range(idx, start, end);
                let _ = engine;
                return Ok(());
            }
            Ok(r) if r.status().as_u16() == 503 => {
                // Receiver paused: wait without counting an attempt.
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            _ => {
                attempts += 1;
                if attempts > engine.cfg.retry_max_attempts {
                    return Err(ErrorCode::NetUnreachable);
                }
                let backoff = engine.cfg.retry_base * 2u32.pow(attempts.min(5));
                tokio::time::sleep(backoff.min(Duration::from_secs(30))).await;
            }
        }
    }
}

/// Session-level pause/resume/cancel (`session.pause` etc.). Works for either direction.
pub async fn control(engine: &Engine, sid: Option<&str>, action: &str) -> Res<Value> {
    let session = active_session(engine, sid)?;
    match action {
        "pause" => {
            if session.state() != St::Transferring {
                return Ok(json!({ "state": session.state().as_str() }));
            }
            session.control.store(CTRL_PAUSE, Ordering::SeqCst);
            set_state(engine, &session, St::Paused);
            Ok(json!({ "state": "paused" }))
        }
        "resume" => {
            if session.state() != St::Paused {
                return Ok(json!({ "state": session.state().as_str() }));
            }
            session.control.store(CTRL_RUN, Ordering::SeqCst);
            set_state(engine, &session, St::Transferring);
            Ok(json!({ "state": "transferring" }))
        }
        "cancel" => {
            if session.state().is_terminal() {
                return Ok(json!({ "cancelled": false, "state": session.state().as_str() }));
            }
            match session.dir {
                Dir::In => {
                    cancel_incoming(engine, &session);
                }
                Dir::Out => {
                    session.control.store(CTRL_CANCEL, Ordering::SeqCst);
                    if session.state() == St::Offered || session.state() == St::Paused || session.state() == St::Transferring {
                        // The driver notices the flag and tells the receiver; finish here if it is not running.
                        let addr = session.peer_addr;
                        if let (Some(addr), fp) = (addr, session.peer_fp.clone()) {
                            if let Ok(client) = crate::identity::mtls_client(&engine.identity, &fp, Duration::from_secs(5)) {
                                let url = peer_url(addr, &format!("/api/v1/session/{}/cancel", session.sid));
                                let _ = client.post(url).send().await;
                            }
                        }
                        finish(engine, &session, St::Cancelled);
                    }
                }
            }
            Ok(json!({ "cancelled": true }))
        }
        _ => Err(err(ErrorCode::InvalidParams, "unknown action")),
    }
}

/// Remote cancel from the peer: `POST /api/v1/session/{sid}/cancel`.
pub fn remote_cancel(engine: &Engine, sid: &str) -> Result<(), (u16, String)> {
    let session = lock(&engine.sessions).get(sid).cloned().ok_or((404, "unknown session".into()))?;
    if session.state().is_terminal() {
        return Ok(());
    }
    if session.dir == Dir::In {
        cancel_incoming(engine, &session);
    } else {
        session.control.store(CTRL_CANCEL, Ordering::SeqCst);
        finish(engine, &session, St::Cancelled);
    }
    Ok(())
}

pub fn history(engine: &Engine) -> Res<Value> {
    let rows = engine.db.sessions_recent(200).map_err(internal)?;
    Ok(json!({ "sessions": rows }))
}

pub fn list_active(engine: &Engine) -> Value {
    let map = lock(&engine.sessions);
    let rows: Vec<Value> = map.values().filter(|s| !s.state().is_terminal()).map(|s| sessions_json_row(s)).collect();
    json!({ "sessions": rows })
}

/// Reads the `Content-Range` header (`bytes a-b/total`). Returns (start, end_inclusive, total).
pub fn parse_content_range(v: &str) -> Option<(u64, u64, u64)> {
    let rest = v.strip_prefix("bytes ")?;
    let (range, total) = rest.split_once('/')?;
    let (s, e) = range.split_once('-')?;
    Some((s.trim().parse().ok()?, e.trim().parse().ok()?, total.trim().parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_merge_has_no_gaps_or_overlaps() {
        let mut r = Vec::new();
        add_range(&mut r, 10, 20);
        add_range(&mut r, 0, 5);
        add_range(&mut r, 5, 10);
        assert_eq!(r, vec![(0, 20)]);
        add_range(&mut r, 30, 40);
        add_range(&mut r, 25, 35);
        assert_eq!(r, vec![(0, 20), (25, 40)]);
        assert_eq!(covered(&r), 35);
        assert_eq!(missing(&r, 50), vec![(20, 25), (40, 50)]);
        assert_eq!(contiguous_prefix(&r), 20);
    }

    #[test]
    fn missing_handles_empty_and_complete() {
        assert_eq!(missing(&[], 10), vec![(0, 10)]);
        assert!(missing(&[(0, 10)], 10).is_empty());
        assert_eq!(missing(&[(0, 4), (6, 10)], 10), vec![(4, 6)]);
    }

    #[test]
    fn content_range_parses() {
        assert_eq!(parse_content_range("bytes 0-99/1000"), Some((0, 99, 1000)));
        assert_eq!(parse_content_range("bytes 0-99"), None);
        assert_eq!(parse_content_range("items 0-1/2"), None);
    }

    #[test]
    fn sid_validation() {
        assert!(is_valid_sid("3f2a-9c"));
        assert!(!is_valid_sid("../etc"));
        assert!(!is_valid_sid(""));
    }
}
