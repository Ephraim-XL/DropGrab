//! SQLite storage: schema from dropgrab-app-design.md section 7.1, plus additive columns and
//! tables that the engine needs (noted in PROGRESS.md). One connection behind a mutex; all
//! queries are short, and callers never hold the guard across an `.await`.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::{params, Connection, OptionalExtension, Row};

pub const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS items (
  id TEXT PRIMARY KEY,
  path TEXT NOT NULL,
  display_name TEXT NOT NULL,
  mime TEXT,
  kind TEXT,
  size INTEGER,
  sha256 TEXT,
  origin TEXT NOT NULL,
  source_url TEXT,
  source_device TEXT,
  duration_ms INTEGER,
  thumbnail TEXT,
  created_at INTEGER NOT NULL,
  last_opened INTEGER,
  favorite INTEGER DEFAULT 0
);
CREATE TABLE IF NOT EXISTS download_jobs (
  id TEXT PRIMARY KEY,
  url TEXT NOT NULL,
  kind TEXT,
  state TEXT,
  filename TEXT,
  destination TEXT,
  total_bytes INTEGER,
  done_bytes INTEGER,
  headers_json TEXT,
  cookies_ref TEXT,
  priority INTEGER,
  retry_count INTEGER,
  group_id TEXT,
  extractor_json TEXT,
  post_json TEXT,
  created_at INTEGER,
  updated_at INTEGER,
  error TEXT,
  dest_dir TEXT,
  validator TEXT,
  quality TEXT,
  source_page TEXT
);
CREATE TABLE IF NOT EXISTS segments (
  job_id TEXT, idx INTEGER, start INTEGER, end INTEGER, done INTEGER,
  PRIMARY KEY (job_id, idx)
);
CREATE TABLE IF NOT EXISTS fragments (
  job_id TEXT, idx INTEGER, url TEXT, done INTEGER, size INTEGER,
  PRIMARY KEY (job_id, idx)
);
CREATE TABLE IF NOT EXISTS devices (
  device_id TEXT PRIMARY KEY, alias TEXT, device_type TEXT, os TEXT,
  fingerprint TEXT, trust TEXT, first_seen INTEGER, last_seen INTEGER,
  allow_remote_download INTEGER DEFAULT 0, auto_accept INTEGER DEFAULT 0
);
CREATE TABLE IF NOT EXISTS sessions (
  sid TEXT PRIMARY KEY, direction TEXT, peer_id TEXT, state TEXT,
  total_bytes INTEGER, done_bytes INTEGER, started_at INTEGER, ended_at INTEGER, error TEXT,
  peer_name TEXT
);
CREATE TABLE IF NOT EXISTS session_files (
  sid TEXT, fid TEXT, name TEXT, size INTEGER, sha256 TEXT, state TEXT, item_id TEXT,
  PRIMARY KEY (sid, fid)
);
CREATE TABLE IF NOT EXISTS rules (
  id TEXT PRIMARY KEY, match_json TEXT, action_json TEXT, enabled INTEGER
);
CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
"#;

pub struct Db {
    conn: Mutex<Connection>,
}

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("database: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct ItemRow {
    pub id: String,
    pub path: String,
    pub display_name: String,
    pub mime: Option<String>,
    pub kind: String,
    pub size: i64,
    pub sha256: Option<String>,
    pub origin: String,
    pub source_url: Option<String>,
    pub source_device: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct DeviceRow {
    pub device_id: String,
    pub alias: String,
    pub device_type: String,
    pub os: String,
    pub fingerprint: String,
    pub trust: String,
    pub first_seen: i64,
    pub last_seen: i64,
    pub allow_remote_download: bool,
    pub auto_accept: bool,
}

#[derive(Debug, Clone)]
pub struct JobRow {
    pub id: String,
    pub url: String,
    pub kind: String,
    pub state: String,
    pub filename: String,
    pub destination: String,
    pub total: i64,
    pub done: i64,
    pub priority: i64,
    pub retry_count: i64,
    pub group_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub error: Option<String>,
    pub dest_dir: String,
    pub validator: Option<String>,
    pub quality: Option<String>,
    pub source_page: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SegmentRow {
    pub idx: i64,
    pub start: i64,
    pub end: i64,
    pub done: i64,
}

const JOB_COLS: &str = "id, url, kind, state, filename, destination, total_bytes, done_bytes, \
    priority, retry_count, group_id, created_at, updated_at, error, dest_dir, validator, quality, \
    source_page";

const ITEM_COLS: &str = "id, path, display_name, mime, kind, size, sha256, origin, source_url, \
    source_device, created_at";

fn job_from_row(r: &Row<'_>) -> rusqlite::Result<JobRow> {
    Ok(JobRow {
        id: r.get(0)?,
        url: r.get(1)?,
        kind: r.get(2)?,
        state: r.get(3)?,
        filename: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
        destination: r.get::<_, Option<String>>(5)?.unwrap_or_default(),
        total: r.get::<_, Option<i64>>(6)?.unwrap_or(0),
        done: r.get::<_, Option<i64>>(7)?.unwrap_or(0),
        priority: r.get::<_, Option<i64>>(8)?.unwrap_or(0),
        retry_count: r.get::<_, Option<i64>>(9)?.unwrap_or(0),
        group_id: r.get(10)?,
        created_at: r.get::<_, Option<i64>>(11)?.unwrap_or(0),
        updated_at: r.get::<_, Option<i64>>(12)?.unwrap_or(0),
        error: r.get(13)?,
        dest_dir: r.get::<_, Option<String>>(14)?.unwrap_or_default(),
        validator: r.get(15)?,
        quality: r.get(16)?,
        source_page: r.get(17)?,
    })
}

fn item_from_row(r: &Row<'_>) -> rusqlite::Result<ItemRow> {
    Ok(ItemRow {
        id: r.get(0)?,
        path: r.get(1)?,
        display_name: r.get(2)?,
        mime: r.get(3)?,
        kind: r.get::<_, Option<String>>(4)?.unwrap_or_else(|| "other".into()),
        size: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
        sha256: r.get(6)?,
        origin: r.get(7)?,
        source_url: r.get(8)?,
        source_device: r.get(9)?,
        created_at: r.get(10)?,
    })
}

fn device_from_row(r: &Row<'_>) -> rusqlite::Result<DeviceRow> {
    Ok(DeviceRow {
        device_id: r.get(0)?,
        alias: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
        device_type: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
        os: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
        fingerprint: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
        trust: r.get::<_, Option<String>>(5)?.unwrap_or_else(|| "seen".into()),
        first_seen: r.get::<_, Option<i64>>(6)?.unwrap_or(0),
        last_seen: r.get::<_, Option<i64>>(7)?.unwrap_or(0),
        allow_remote_download: r.get::<_, Option<i64>>(8)?.unwrap_or(0) != 0,
        auto_accept: r.get::<_, Option<i64>>(9)?.unwrap_or(0) != 0,
    })
}

impl Db {
    /// Opens the database, applies migrations (after a backup of an existing file), and sets
    /// WAL mode. Migrations are forward-only (section 3.4).
    pub fn open(path: &Path) -> Result<Db, DbError> {
        let existed = path.exists();
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < SCHEMA_VERSION {
            if existed && version == 0 && path.metadata()?.len() > 0 {
                // Back up before the first migration of a populated file.
                let backup = path.with_extension("db.bak");
                std::fs::copy(path, backup)?;
            }
            conn.execute_batch(SCHEMA)?;
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        Ok(Db { conn: Mutex::new(conn) })
    }

    pub fn open_memory() -> Result<Db, DbError> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Db { conn: Mutex::new(conn) })
    }

    fn c(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    // ----- meta -------------------------------------------------------------------------

    pub fn meta_get(&self, key: &str) -> Result<Option<String>, DbError> {
        Ok(self
            .c()
            .query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| r.get(0))
            .optional()?)
    }

    pub fn meta_set(&self, key: &str, value: &str) -> Result<(), DbError> {
        self.c().execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // ----- settings ---------------------------------------------------------------------

    pub fn settings_all(&self) -> Result<Vec<(String, String)>, DbError> {
        let c = self.c();
        let mut stmt = c.prepare("SELECT key, value FROM settings")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn setting_put(&self, key: &str, value: &str) -> Result<(), DbError> {
        self.c().execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // ----- library items ----------------------------------------------------------------

    pub fn item_insert(&self, it: &ItemRow) -> Result<(), DbError> {
        self.c().execute(
            "INSERT INTO items (id, path, display_name, mime, kind, size, sha256, origin, \
             source_url, source_device, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                it.id, it.path, it.display_name, it.mime, it.kind, it.size, it.sha256, it.origin,
                it.source_url, it.source_device, it.created_at
            ],
        )?;
        Ok(())
    }

    pub fn item_get(&self, id: &str) -> Result<Option<ItemRow>, DbError> {
        Ok(self
            .c()
            .query_row(
                &format!("SELECT {ITEM_COLS} FROM items WHERE id = ?1"),
                params![id],
                item_from_row,
            )
            .optional()?)
    }

    pub fn items_all(&self) -> Result<Vec<ItemRow>, DbError> {
        let c = self.c();
        let mut stmt = c.prepare(&format!("SELECT {ITEM_COLS} FROM items ORDER BY created_at DESC"))?;
        let rows = stmt.query_map([], item_from_row)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn item_delete(&self, id: &str) -> Result<(), DbError> {
        self.c().execute("DELETE FROM items WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn item_set_path(&self, id: &str, path: &str, display_name: &str) -> Result<(), DbError> {
        self.c().execute(
            "UPDATE items SET path = ?2, display_name = ?3 WHERE id = ?1",
            params![id, path, display_name],
        )?;
        Ok(())
    }

    pub fn item_set_sha(&self, id: &str, sha: &str) -> Result<(), DbError> {
        self.c().execute("UPDATE items SET sha256 = ?2 WHERE id = ?1", params![id, sha])?;
        Ok(())
    }

    // ----- devices ----------------------------------------------------------------------

    pub fn device_get(&self, id: &str) -> Result<Option<DeviceRow>, DbError> {
        Ok(self
            .c()
            .query_row(
                "SELECT device_id, alias, device_type, os, fingerprint, trust, first_seen, \
                 last_seen, allow_remote_download, auto_accept FROM devices WHERE device_id = ?1",
                params![id],
                device_from_row,
            )
            .optional()?)
    }

    pub fn devices_all(&self) -> Result<Vec<DeviceRow>, DbError> {
        let c = self.c();
        let mut stmt = c.prepare(
            "SELECT device_id, alias, device_type, os, fingerprint, trust, first_seen, \
             last_seen, allow_remote_download, auto_accept FROM devices ORDER BY last_seen DESC",
        )?;
        let rows = stmt.query_map([], device_from_row)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn device_upsert(&self, d: &DeviceRow) -> Result<(), DbError> {
        self.c().execute(
            "INSERT INTO devices (device_id, alias, device_type, os, fingerprint, trust, first_seen, \
             last_seen, allow_remote_download, auto_accept) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) \
             ON CONFLICT(device_id) DO UPDATE SET alias=excluded.alias, device_type=excluded.device_type, \
             os=excluded.os, fingerprint=excluded.fingerprint, trust=excluded.trust, \
             last_seen=excluded.last_seen",
            params![
                d.device_id, d.alias, d.device_type, d.os, d.fingerprint, d.trust, d.first_seen,
                d.last_seen, d.allow_remote_download as i64, d.auto_accept as i64
            ],
        )?;
        Ok(())
    }

    pub fn device_set_trust(&self, id: &str, trust: &str, auto_accept: Option<bool>) -> Result<(), DbError> {
        let c = self.c();
        c.execute("UPDATE devices SET trust = ?2 WHERE device_id = ?1", params![id, trust])?;
        if let Some(a) = auto_accept {
            c.execute("UPDATE devices SET auto_accept = ?2 WHERE device_id = ?1", params![id, a as i64])?;
        }
        Ok(())
    }

    pub fn device_set_allowance(&self, id: &str, auto_accept: Option<bool>, allow_remote: Option<bool>) -> Result<(), DbError> {
        let c = self.c();
        if let Some(a) = auto_accept {
            c.execute("UPDATE devices SET auto_accept = ?2 WHERE device_id = ?1", params![id, a as i64])?;
        }
        if let Some(r) = allow_remote {
            c.execute(
                "UPDATE devices SET allow_remote_download = ?2 WHERE device_id = ?1",
                params![id, r as i64],
            )?;
        }
        Ok(())
    }

    // ----- download jobs ----------------------------------------------------------------

    pub fn job_insert(&self, j: &JobRow) -> Result<(), DbError> {
        self.c().execute(
            "INSERT INTO download_jobs (id, url, kind, state, filename, destination, total_bytes, \
             done_bytes, priority, retry_count, group_id, created_at, updated_at, error, dest_dir, \
             validator, quality, source_page) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
            params![
                j.id, j.url, j.kind, j.state, j.filename, j.destination, j.total, j.done, j.priority,
                j.retry_count, j.group_id, j.created_at, j.updated_at, j.error, j.dest_dir,
                j.validator, j.quality, j.source_page
            ],
        )?;
        Ok(())
    }

    pub fn job_get(&self, id: &str) -> Result<Option<JobRow>, DbError> {
        Ok(self
            .c()
            .query_row(
                &format!("SELECT {JOB_COLS} FROM download_jobs WHERE id = ?1"),
                params![id],
                job_from_row,
            )
            .optional()?)
    }

    pub fn jobs_all(&self) -> Result<Vec<JobRow>, DbError> {
        let c = self.c();
        let mut stmt = c.prepare(&format!(
            "SELECT {JOB_COLS} FROM download_jobs ORDER BY priority DESC, created_at ASC"
        ))?;
        let rows = stmt.query_map([], job_from_row)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn job_set_state(&self, id: &str, state: &str, error: Option<&str>, now: i64) -> Result<(), DbError> {
        self.c().execute(
            "UPDATE download_jobs SET state = ?2, error = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, state, error, now],
        )?;
        Ok(())
    }

    pub fn job_set_meta(
        &self,
        id: &str,
        total: i64,
        filename: &str,
        destination: &str,
        validator: Option<&str>,
        url: &str,
    ) -> Result<(), DbError> {
        self.c().execute(
            "UPDATE download_jobs SET total_bytes = ?2, filename = ?3, destination = ?4, \
             validator = ?5, url = ?6 WHERE id = ?1",
            params![id, total, filename, destination, validator, url],
        )?;
        Ok(())
    }

    pub fn job_set_progress(&self, id: &str, done: i64, now: i64) -> Result<(), DbError> {
        self.c().execute(
            "UPDATE download_jobs SET done_bytes = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, done, now],
        )?;
        Ok(())
    }

    pub fn job_set_priority(&self, id: &str, priority: i64) -> Result<(), DbError> {
        self.c().execute("UPDATE download_jobs SET priority = ?2 WHERE id = ?1", params![id, priority])?;
        Ok(())
    }

    pub fn job_bump_retry(&self, id: &str) -> Result<(), DbError> {
        self.c().execute(
            "UPDATE download_jobs SET retry_count = retry_count + 1 WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn job_delete(&self, id: &str) -> Result<(), DbError> {
        let c = self.c();
        c.execute("DELETE FROM segments WHERE job_id = ?1", params![id])?;
        c.execute("DELETE FROM download_jobs WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn segments_replace(&self, job_id: &str, segs: &[SegmentRow]) -> Result<(), DbError> {
        let mut c = self.c();
        let tx = c.transaction()?;
        tx.execute("DELETE FROM segments WHERE job_id = ?1", params![job_id])?;
        for s in segs {
            tx.execute(
                "INSERT INTO segments (job_id, idx, start, end, done) VALUES (?1,?2,?3,?4,?5)",
                params![job_id, s.idx, s.start, s.end, s.done],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn segments_get(&self, job_id: &str) -> Result<Vec<SegmentRow>, DbError> {
        let c = self.c();
        let mut stmt = c.prepare("SELECT idx, start, end, done FROM segments WHERE job_id = ?1 ORDER BY idx")?;
        let rows = stmt
            .query_map(params![job_id], |r| {
                Ok(SegmentRow { idx: r.get(0)?, start: r.get(1)?, end: r.get(2)?, done: r.get(3)? })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Checkpoint: segment done offsets written in one transaction (section 3.4).
    pub fn segments_checkpoint(&self, job_id: &str, segs: &[SegmentRow]) -> Result<(), DbError> {
        let mut c = self.c();
        let tx = c.transaction()?;
        for s in segs {
            tx.execute(
                "UPDATE segments SET done = ?3 WHERE job_id = ?1 AND idx = ?2",
                params![job_id, s.idx, s.done],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // ----- sessions ---------------------------------------------------------------------

    pub fn session_upsert(
        &self,
        sid: &str,
        direction: &str,
        peer_id: &str,
        peer_name: &str,
        state: &str,
        total: i64,
        done: i64,
        started_at: i64,
        ended_at: Option<i64>,
        error: Option<&str>,
    ) -> Result<(), DbError> {
        self.c().execute(
            "INSERT INTO sessions (sid, direction, peer_id, peer_name, state, total_bytes, done_bytes, \
             started_at, ended_at, error) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) \
             ON CONFLICT(sid) DO UPDATE SET state=excluded.state, done_bytes=excluded.done_bytes, \
             ended_at=excluded.ended_at, error=excluded.error",
            params![sid, direction, peer_id, peer_name, state, total, done, started_at, ended_at, error],
        )?;
        Ok(())
    }

    pub fn session_file_upsert(&self, sid: &str, fid: &str, name: &str, size: i64, sha: Option<&str>, state: &str, item_id: Option<&str>) -> Result<(), DbError> {
        self.c().execute(
            "INSERT INTO session_files (sid, fid, name, size, sha256, state, item_id) VALUES (?1,?2,?3,?4,?5,?6,?7) \
             ON CONFLICT(sid, fid) DO UPDATE SET state=excluded.state, sha256=excluded.sha256, item_id=excluded.item_id",
            params![sid, fid, name, size, sha, state, item_id],
        )?;
        Ok(())
    }

    pub fn sessions_recent(&self, limit: i64) -> Result<Vec<serde_json::Value>, DbError> {
        let c = self.c();
        let mut stmt = c.prepare(
            "SELECT sid, direction, peer_id, peer_name, state, total_bytes, done_bytes, started_at, \
             ended_at, error FROM sessions ORDER BY started_at DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit], |r| {
                Ok(serde_json::json!({
                    "sid": r.get::<_, String>(0)?,
                    "direction": r.get::<_, Option<String>>(1)?,
                    "peerId": r.get::<_, Option<String>>(2)?,
                    "peerName": r.get::<_, Option<String>>(3)?,
                    "state": r.get::<_, Option<String>>(4)?,
                    "totalBytes": r.get::<_, Option<i64>>(5)?,
                    "doneBytes": r.get::<_, Option<i64>>(6)?,
                    "startedAt": r.get::<_, Option<i64>>(7)?,
                    "endedAt": r.get::<_, Option<i64>>(8)?,
                    "error": r.get::<_, Option<String>>(9)?,
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

/// Used by tests and the CLI to check that an optional row exists without an error.
pub fn exists(db: &Db, sql: &str, id: &str) -> Result<bool, DbError> {
    let n: i64 = db.c().query_row(sql, params![id], |r| r.get(0))?;
    Ok(n > 0)
}
