//! Staged uploads for browser clients: the `files.stage` control-API command.
//!
//! A web page cannot hand the daemon a local filesystem path (the file picker does not expose
//! one), so the client posts the file bytes as base64 and gets back an id and a path. The path
//! can be used directly in `session.send` / `transfer.start` items, or the id can be passed as
//! `stagedId`.
//!
//! Storage: `<data-dir>/staging/<id>/<sanitized name>`. The id is random; the name never
//! contains a path separator, and the joined path is checked to stay inside the id's folder.
//! A staged file is removed when a transfer that used it ends, or after [`STAGE_TTL`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine as _;
use dg_types::ErrorCode;
use serde_json::{json, Value};

use crate::engine::{err, internal, lock, Engine, Res};
use crate::util::{now_ms, random_hex, sanitize_filename};

/// Subfolder of the data directory holding staged files.
pub const STAGE_DIR: &str = "staging";

/// Largest file that can be staged in one command. Matches the peer protocol's maximum upload
/// chunk (`peer::MAX_CHUNK`); a browser can stage anything up to this and send it normally.
pub const MAX_STAGE_BYTES: usize = 8 * 1024 * 1024;

/// Longest accepted `name` (characters), matching the sanitizer's own cap.
pub const MAX_STAGE_NAME: usize = 200;

/// How long an unused staged file survives before the sweep removes it.
pub const STAGE_TTL: Duration = Duration::from_secs(60 * 60);

/// The pump runs at 4 Hz; the staging sweep only scans every 10 s.
const SWEEP_EVERY_MS: i64 = 10_000;

#[derive(Clone, Debug)]
pub struct Staged {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub created_ms: i64,
}

impl Staged {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "name": self.name,
            "size": self.size,
            "path": self.path.display().to_string(),
        })
    }
}

#[derive(Default)]
pub struct StagingState {
    pub files: HashMap<String, Staged>,
    pub last_sweep_ms: i64,
}

/// Validates a client-supplied name. Path separators and `..` are rejected outright (a staged
/// name is one path segment, never a path), then the usual sanitiser runs for the rest.
pub fn stage_name(raw: &str) -> Res<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(err(ErrorCode::InvalidParams, "name is required"));
    }
    if raw.chars().count() > MAX_STAGE_NAME {
        return Err(err(
            ErrorCode::InvalidParams,
            format!("name is longer than {MAX_STAGE_NAME} characters"),
        ));
    }
    if raw.contains('/') || raw.contains('\\') {
        return Err(err(
            ErrorCode::InvalidParams,
            "name must not contain a path separator (send a file name, not a path)",
        ));
    }
    if raw == "." || raw == ".." {
        return Err(err(ErrorCode::InvalidParams, "name must be a file name"));
    }
    let name = sanitize_filename(raw);
    // Belt and braces: whatever the sanitiser produced must still be a single safe segment.
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
    {
        return Err(err(ErrorCode::InvalidParams, "name is not a usable file name"));
    }
    Ok(name)
}

/// `files.stage`: `{ name, content (base64), mime? }` → `{ id, name, size, path }`.
pub async fn stage(engine: &Engine, p: &Value) -> Res<Value> {
    let raw_name = p
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| err(ErrorCode::InvalidParams, "name is required"))?;
    let name = stage_name(raw_name)?;
    let content = p
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| err(ErrorCode::InvalidParams, "content (base64) is required"))?;

    let too_large = || {
        err(
            ErrorCode::PayloadTooLarge,
            format!("file is larger than the {} MiB staging limit", MAX_STAGE_BYTES / (1024 * 1024)),
        )
    };
    // Base64 uses 4 characters per 3 bytes; reject before allocating the decoded buffer.
    if content.len() > MAX_STAGE_BYTES.div_ceil(3) * 4 + 4 {
        return Err(too_large());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(content.as_bytes())
        .map_err(|e| err(ErrorCode::InvalidParams, format!("content is not valid base64: {e}")))?;
    if bytes.len() > MAX_STAGE_BYTES {
        return Err(too_large());
    }

    let id = format!("stg_{}", random_hex(16));
    let dir = engine.cfg.data_dir.join(STAGE_DIR).join(&id);
    let path = dir.join(&name);
    // The joined path must be exactly one level under the id's folder.
    if path.parent() != Some(dir.as_path()) {
        return Err(err(ErrorCode::InvalidParams, "name is not a usable file name"));
    }
    tokio::fs::create_dir_all(&dir).await.map_err(internal)?;
    if let Err(e) = tokio::fs::write(&path, &bytes).await {
        let _ = tokio::fs::remove_dir_all(&dir).await;
        return Err(internal(e));
    }

    let staged = Staged { id: id.clone(), name, path, size: bytes.len() as u64, created_ms: now_ms() };
    lock(&engine.staging).files.insert(id, staged.clone());
    Ok(staged.to_json())
}

/// Looks up a staged file by id.
pub fn get(engine: &Engine, id: &str) -> Option<Staged> {
    lock(&engine.staging).files.get(id).cloned()
}

/// Removes staged files (and their folders). Called when a transfer that used them ends.
pub fn release(engine: &Engine, ids: &[String]) {
    if ids.is_empty() {
        return;
    }
    let mut removed = Vec::new();
    {
        let mut st = lock(&engine.staging);
        for id in ids {
            if let Some(f) = st.files.remove(id) {
                removed.push(f.path);
            }
        }
    }
    for path in removed {
        if let Some(dir) = path.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// Removes staged files older than [`STAGE_TTL`]. Cheap to call from the pump; scans at most
/// every 10 s.
pub fn sweep(engine: &Engine) {
    let now = now_ms();
    let ttl_ms = STAGE_TTL.as_millis() as i64;
    let mut expired = Vec::new();
    {
        let mut st = lock(&engine.staging);
        if now.saturating_sub(st.last_sweep_ms) < SWEEP_EVERY_MS {
            return;
        }
        st.last_sweep_ms = now;
        let ids: Vec<String> = st
            .files
            .values()
            .filter(|f| now.saturating_sub(f.created_ms) > ttl_ms)
            .map(|f| f.id.clone())
            .collect();
        for id in ids {
            if let Some(f) = st.files.remove(&id) {
                expired.push(f.path);
            }
        }
    }
    for path in expired {
        if let Some(dir) = path.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// Creates the staging folder and removes folders left behind by a previous run (a restart
/// loses the in-memory registry, so those files can never be sent again).
pub fn init(data_dir: &Path) {
    let root = data_dir.join(STAGE_DIR);
    if std::fs::create_dir_all(&root).is_err() {
        return;
    }
    sweep_orphans(&root);
}

fn sweep_orphans(root: &Path) {
    let ttl_ms = STAGE_TTL.as_millis() as i64;
    let now = now_ms();
    let Ok(entries) = std::fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let age_ms = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| now - d.as_millis() as i64)
            .unwrap_or(0);
        if age_ms > ttl_ms {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversal_names_are_rejected() {
        for bad in ["../evil.txt", "..", ".", "a/b.txt", "a\\b.txt", "/etc/passwd", "C:\\x.txt", "   "] {
            assert!(stage_name(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn ordinary_names_are_kept_and_sanitized() {
        assert_eq!(stage_name("photo.jpg").unwrap(), "photo.jpg");
        assert_eq!(stage_name("  notes.txt  ").unwrap(), "notes.txt");
        assert_eq!(stage_name("a:b*c?.txt").unwrap(), "a_b_c_.txt");
        assert_eq!(stage_name("a..b.txt").unwrap(), "a..b.txt");
        let long = "x".repeat(MAX_STAGE_NAME);
        assert!(stage_name(&long).is_ok(), "exactly at the limit is allowed");
        assert!(stage_name(&"x".repeat(MAX_STAGE_NAME + 1)).is_err());
    }
}
