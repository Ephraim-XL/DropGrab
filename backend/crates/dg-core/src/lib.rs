//! dg-core: the DropGrab engine.
//!
//! `Backend` is the in-process handle (section 3.7). The desktop daemon wraps it with a loopback
//! control server; mobile hosts would call `invoke` directly through FFI.

// One audited `unsafe` block exists: the `statvfs` call in `engine::free_bytes`, which has a
// safety comment. Everything else is safe code (section 3.10).
#![deny(unsafe_op_in_unsafe_fn)]

pub mod api;
pub mod db;
pub mod discovery;
pub mod downloads;
pub mod engine;
pub mod events;
pub mod extractor;
pub mod identity;
pub mod library;
pub mod peer;
pub mod settings;
pub mod staging;
pub mod transfer;
pub mod util;
pub mod webshare;

use std::path::PathBuf;

pub use engine::{Config, Engine};

/// In-process handle: start, invoke commands, read events, stop.
#[derive(Clone)]
pub struct Backend {
    pub engine: Engine,
}

impl Backend {
    pub async fn start(cfg: Config) -> Result<Backend, String> {
        let engine = Engine::open(cfg).await?;
        downloads::load_jobs(&engine);
        Ok(Backend { engine })
    }

    /// Runs one command. The response shape matches the JSON-RPC body in section 3.5.
    pub async fn invoke(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, dg_types::ApiError> {
        api::dispatch(&self.engine, method, params).await
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<dg_types::Event> {
        self.engine.bus.subscribe()
    }

    pub async fn stop(&self) {
        self.engine.stop().await;
    }
}

/// Default data directory for a desktop user, following section 3.5 and the platform norms.
pub fn default_data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("DROPGRAB_DATA_DIR") {
        return PathBuf::from(d);
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(h) = std::env::var_os("HOME") {
            return PathBuf::from(h).join("Library/Application Support/DropGrab");
        }
    }
    if let Some(x) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(x).join("dropgrab");
    }
    if let Some(h) = std::env::var_os("HOME") {
        return PathBuf::from(h).join(".local/share/dropgrab");
    }
    PathBuf::from("dropgrab-data")
}
