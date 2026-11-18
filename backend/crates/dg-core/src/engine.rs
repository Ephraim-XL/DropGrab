//! Engine: owns all service state and is shared by the control API, the peer server, discovery,
//! the download manager and web share. Each service is a module with its own `impl Engine`
//! block; state is held behind small mutexes that are never held across `.await`.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use dg_types::{ApiError, ErrorCode};
use serde_json::{json, Value};
use tokio::sync::Notify;

use crate::db::{Db, DeviceRow};
use crate::{discovery, downloads, transfer};
use crate::events::EventBus;
use crate::identity::{self, Identity};
use crate::settings::Settings;
use crate::util::now_ms;

#[derive(Clone, Debug)]
pub struct Config {
    pub data_dir: PathBuf,
    pub downloads_dir: PathBuf,
    pub device_name: String,
    pub device_type: String,
    /// Peer HTTPS port (section 4.4). 0 binds an ephemeral port (tests).
    pub peer_port: u16,
    /// How many following ports to try if `peer_port` is taken (Appendix B: up to 53328).
    pub peer_port_fallbacks: u16,
    /// Address the peer server binds. The LAN default is 0.0.0.0.
    pub peer_bind: IpAddr,
    /// Address written into QR payloads and announcements. None = detected local address.
    pub advertise_ip: Option<IpAddr>,
    pub discovery: bool,
    pub multicast_group: Ipv4Addr,
    pub discovery_port: u16,
    pub discovery_interval: Duration,
    pub webshare_port: u16,
    pub retry_base: Duration,
    pub retry_max_attempts: u32,
    pub network_retry_window: Duration,
    pub offer_timeout: Duration,
    pub pair_ttl: Duration,
    pub webshare_ttl: Duration,
    pub stale_after: Duration,
    pub remove_after: Duration,
    pub chunk_size: u64,
    pub streams: u32,
}

impl Config {
    /// Defaults for a desktop daemon. `data_dir` holds the database, keys, and part files.
    pub fn desktop(data_dir: PathBuf, downloads_dir: PathBuf) -> Config {
        let host = std::env::var("HOSTNAME")
            .ok()
            .filter(|h| !h.is_empty())
            .unwrap_or_else(|| "DropGrab device".into());
        Config {
            data_dir,
            downloads_dir,
            device_name: host,
            device_type: "desktop".into(),
            peer_port: 53318,
            peer_port_fallbacks: 10,
            peer_bind: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            advertise_ip: None,
            discovery: true,
            multicast_group: Ipv4Addr::new(239, 255, 42, 99),
            discovery_port: 53318,
            discovery_interval: Duration::from_secs(10),
            webshare_port: 53320,
            retry_base: Duration::from_secs(1),
            retry_max_attempts: 10,
            network_retry_window: Duration::from_secs(120),
            offer_timeout: Duration::from_secs(60),
            pair_ttl: Duration::from_secs(120),
            webshare_ttl: Duration::from_secs(600),
            stale_after: Duration::from_secs(30),
            remove_after: Duration::from_secs(120),
            chunk_size: 4 * 1024 * 1024,
            streams: 2,
        }
    }
}

/// A peer seen on the network (discovery table, section 4.2).
#[derive(Clone, Debug)]
pub struct Peer {
    pub device_id: String,
    pub alias: String,
    pub device_type: String,
    pub os: String,
    pub fingerprint: String,
    pub ip: IpAddr,
    pub port: u16,
    pub last_seen_ms: i64,
    pub link: String,
}

pub struct PairingState {
    pub pin: String,
    pub expires_ms: i64,
    pub used: bool,
    /// Wrong PINs seen on this pairing session. Scoped to the session (a new `pair.qr_create`
    /// resets it) so an attacker can only lock out the code they are guessing at.
    pub failed_pins: u32,
    pub uri: String,
}

pub struct RemoteRequest {
    pub id: String,
    pub from_device: String,
    pub from_name: String,
    pub url: String,
    pub job_id: Option<String>,
    pub at_ms: i64,
}

pub struct Inner {
    pub cfg: Config,
    pub db: Db,
    pub identity: Identity,
    pub settings: Mutex<Settings>,
    pub bus: EventBus,
    pub peers: Mutex<HashMap<String, Peer>>,
    pub sessions: Mutex<HashMap<String, Arc<crate::transfer::Session>>>,
    pub jobs: Mutex<HashMap<String, Arc<crate::downloads::Job>>>,
    pub pairing: Mutex<Option<PairingState>>,
    pub remote_requests: Mutex<Vec<RemoteRequest>>,
    pub webshare: Mutex<Option<crate::webshare::ShareHandle>>,
    /// Files staged through `files.stage` (browser uploads), keyed by staging id.
    pub staging: Mutex<crate::staging::StagingState>,
    pub peer_port: u16,
    pub shutdown: Notify,
    pub stopping: AtomicBool,
    pub http: reqwest::Client,
    pub extractor_cache: Mutex<HashMap<String, (i64, Value)>>,
    pub multicast_joined: AtomicBool,
    pub started_ms: i64,
    pub tasks: Mutex<Vec<tokio::task::JoinHandle<()>>>,
    pub peer_server: Mutex<Option<axum_server::Handle<std::net::SocketAddr>>>,
    pub last_command_ms: AtomicU64,
    pub rate_tokens: Mutex<(i64, u32)>,
}

#[derive(Clone)]
pub struct Engine(pub Arc<Inner>);

impl std::ops::Deref for Engine {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.0
    }
}

pub type Res<T> = Result<T, ApiError>;

pub fn err(code: ErrorCode, msg: impl Into<String>) -> ApiError {
    ApiError::new(code, msg)
}

pub fn internal<E: std::fmt::Display>(e: E) -> ApiError {
    ApiError::new(ErrorCode::Internal, e.to_string())
}

pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Engine {
    /// Opens storage, loads or creates the identity, and binds the peer port.
    /// Background services (discovery, peer server, sweeps) are started by `start`.
    pub async fn open(cfg: Config) -> Result<Engine, String> {
        identity::install_crypto_provider();
        std::fs::create_dir_all(&cfg.data_dir).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&cfg.downloads_dir).map_err(|e| e.to_string())?;
        crate::staging::init(&cfg.data_dir);
        let db = Db::open(&cfg.data_dir.join("dropgrab.db")).map_err(|e| e.to_string())?;
        let identity = Identity::load_or_create(&cfg.data_dir.join("identity"), &db)?;
        let default_folder = cfg.downloads_dir.display().to_string();
        let mut settings = Settings::defaults(&cfg.device_name, &default_folder);
        settings = Settings::load(&db, settings)?;
        let peer_listener = bind_peer(&cfg).await?;
        let peer_port = peer_listener.local_addr().map_err(|e| e.to_string())?.port();
        let http = reqwest::Client::builder()
            .user_agent(concat!("DropGrab/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(60 * 10))
            .build()
            .map_err(|e| e.to_string())?;
        let engine = Engine(Arc::new(Inner {
            cfg,
            db,
            identity,
            settings: Mutex::new(settings),
            bus: EventBus::new(),
            peers: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            jobs: Mutex::new(HashMap::new()),
            pairing: Mutex::new(None),
            remote_requests: Mutex::new(Vec::new()),
            webshare: Mutex::new(None),
            staging: Mutex::new(crate::staging::StagingState::default()),
            peer_port,
            shutdown: Notify::new(),
            stopping: AtomicBool::new(false),
            http,
            extractor_cache: Mutex::new(HashMap::new()),
            multicast_joined: AtomicBool::new(false),
            started_ms: now_ms(),
            tasks: Mutex::new(Vec::new()),
            peer_server: Mutex::new(None),
            last_command_ms: AtomicU64::new(0),
            rate_tokens: Mutex::new((0, 0)),
        }));
        engine.start_peer_server(peer_listener)?;
        engine.reconcile_library();
        engine.spawn_pump();
        if engine.cfg.discovery {
            discovery::spawn(&engine);
        }
        Ok(engine)
    }

    pub fn emit(&self, kind: &str, data: Value) {
        self.bus.emit(kind, data);
    }

    pub fn settings(&self) -> Settings {
        lock(&self.settings).clone()
    }

    pub fn device_name(&self) -> String {
        lock(&self.settings).device_name.clone()
    }

    pub fn visibility(&self) -> String {
        lock(&self.settings).visibility.clone()
    }

    pub fn spawn<F>(&self, fut: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let handle = tokio::spawn(fut);
        let mut tasks = lock(&self.tasks);
        tasks.retain(|h| !h.is_finished());
        tasks.push(handle);
    }

    /// Stops background services. Active transfers are cancelled; state is already checkpointed.
    pub async fn stop(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        self.shutdown.notify_waiters();
        if let Some(h) = lock(&self.peer_server).take() {
            h.graceful_shutdown(Some(Duration::from_secs(2)));
        }
        if let Some(h) = lock(&self.webshare).take() {
            h.stop();
        }
        let handles: Vec<_> = lock(&self.tasks).drain(..).collect();
        for h in handles {
            h.abort();
        }
    }

    /// Token-bucket style limit on control commands (section 3.5 rate limits): 100 per second.
    pub fn rate_check(&self) -> Res<()> {
        let now = now_ms();
        let mut b = lock(&self.rate_tokens);
        if now - b.0 >= 1000 {
            *b = (now, 0);
        }
        b.1 += 1;
        if b.1 > 100 {
            return Err(err(ErrorCode::RateLimited, "too many commands per second"));
        }
        Ok(())
    }

    /// Looks up a device record, creating `seen` for an unknown device that is announced.
    pub fn device_row(&self, id: &str) -> Res<Option<DeviceRow>> {
        self.db.device_get(id).map_err(internal)
    }

    pub fn peer_for(&self, device_id: &str) -> Option<Peer> {
        lock(&self.peers).get(device_id).cloned()
    }

    /// Records or refreshes a device from a verified identity (announce, info, pairing).
    /// A changed certificate on a trusted device downgrades it to `seen` and warns (section 4.3).
    pub fn upsert_device(&self, device_id: &str, alias: &str, device_type: &str, os: &str, fingerprint: &str) -> Res<DeviceRow> {
        let now = now_ms() / 1000;
        let existing = self.device_row(device_id)?;
        let mut trust = "seen".to_string();
        let mut first_seen = now;
        let mut auto = false;
        let mut allow_remote = false;
        let mut changed_cert = false;
        if let Some(row) = &existing {
            trust = row.trust.clone();
            first_seen = row.first_seen;
            auto = row.auto_accept;
            allow_remote = row.allow_remote_download;
            if !row.fingerprint.is_empty() && row.fingerprint != fingerprint && row.trust == "trusted" {
                trust = "seen".into();
                changed_cert = true;
            }
        }
        let row = DeviceRow {
            device_id: device_id.to_string(),
            alias: alias.to_string(),
            device_type: device_type.to_string(),
            os: os.to_string(),
            fingerprint: fingerprint.to_string(),
            trust: trust.clone(),
            first_seen,
            last_seen: now,
            allow_remote_download: allow_remote,
            auto_accept: auto,
        };
        self.db.device_upsert(&row).map_err(internal)?;
        if changed_cert {
            self.emit("device.changed", json!({ "deviceId": device_id, "trust": "seen" }));
            self.emit(
                "notification.request",
                json!({ "kind": "cert-changed", "title": format!("{alias} has a new certificate"),
                        "body": "Check the verification code again before trusting it." }),
            );
        }
        Ok(row)
    }

    pub fn device_json(&self, row: &DeviceRow) -> Value {
        let peer = self.peer_for(&row.device_id);
        let online = peer.is_some();
        let link = if online { "same network".to_string() } else { "not seen recently".to_string() };
        json!({
            "id": row.device_id,
            "name": row.alias,
            "type": row.device_type,
            "os": row.os,
            "trust": row.trust,
            "online": online,
            "link": link,
            "lastSeen": crate::util::when_label(row.last_seen),
            "autoAccept": row.auto_accept,
            "allowRemote": row.allow_remote_download,
            "fingerprint": crate::util::display_fingerprint(&row.fingerprint),
        })
    }

    /// Full identity block for requests we send (section 4.4.3 `sender`).
    pub fn self_sender(&self) -> dg_types::Sender {
        dg_types::Sender {
            device_id: self.identity.device_id.clone(),
            alias: self.device_name(),
            device_type: self.cfg.device_type.clone(),
            fingerprint: self.identity.fingerprint.clone(),
        }
    }

    pub fn info(&self) -> dg_types::InfoResponse {
        dg_types::InfoResponse {
            device_id: self.identity.device_id.clone(),
            alias: self.device_name(),
            device_type: self.cfg.device_type.clone(),
            os: std::env::consts::OS.to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: dg_types::PROTOCOL_VERSION,
            capabilities: vec![
                "chunked".into(),
                "parallel".into(),
                "sha256".into(),
                "web-share".into(),
                "remote-download".into(),
            ],
            max_parallel_streams: self.cfg.streams.max(4),
            fingerprint: self.identity.fingerprint.clone(),
        }
    }

    /// Advertised address for QR payloads and announcements.
    pub fn advertise_ip(&self) -> IpAddr {
        self.cfg.advertise_ip.unwrap_or_else(detect_local_ip)
    }

    pub fn is_blocked(&self, device_id: &str, fingerprint: &str) -> bool {
        match self.db.device_get(device_id) {
            Ok(Some(row)) => row.trust == "blocked" || (!fingerprint.is_empty() && row.fingerprint == fingerprint && row.trust == "blocked"),
            _ => false,
        }
    }

    pub fn is_trusted(&self, device_id: &str, fingerprint: &str) -> bool {
        match self.db.device_get(device_id) {
            Ok(Some(row)) => row.trust == "trusted" && row.fingerprint == fingerprint,
            _ => false,
        }
    }

    /// Settings changed on disk by a previous run are reloaded at start; this persists a write.
    pub fn persist_settings(&self, changed: &[String]) -> Res<()> {
        let s = self.settings();
        let json = s.to_json();
        for key in changed {
            if let Some(v) = json.get(key) {
                self.db.setting_put(key, &v.to_string()).map_err(internal)?;
            }
        }
        Ok(())
    }

    /// Startup reconciliation (section 7.4): library rows whose files are gone are removed.
    fn reconcile_library(&self) {
        if let Ok(items) = self.db.items_all() {
            for it in items {
                if !std::path::Path::new(&it.path).exists() {
                    let _ = self.db.item_delete(&it.id);
                    tracing::info!(id = %it.id, "removed stale library entry");
                }
            }
        }
    }

    /// Progress coalescing (section 3.4): one pump task emits progress at most 4 times a second.
    fn spawn_pump(&self) {
        let engine = self.clone();
        self.spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_millis(250));
            loop {
                ticker.tick().await;
                if engine.stopping.load(Ordering::SeqCst) {
                    break;
                }
                pump_once(&engine);
            }
        });
    }

    /// Removes peers not heard from in `remove_after` (section 4.2: stale at 30 s, removed at 120 s).
    pub fn sweep_peers(&self) {
        let now = now_ms();
        let stale_ms = self.cfg.stale_after.as_millis() as i64;
        let remove_ms = self.cfg.remove_after.as_millis() as i64;
        let mut gone = Vec::new();
        {
            let mut peers = lock(&self.peers);
            let trusted: Vec<String> = peers
                .keys()
                .filter(|id| self.is_trusted(id, &peers[*id].fingerprint))
                .cloned()
                .collect();
            peers.retain(|id, p| {
                let age = now - p.last_seen_ms;
                if age > remove_ms && !trusted.contains(id) {
                    gone.push((id.clone(), p.alias_or_name()));
                    false
                } else {
                    true
                }
            });
        }
        for (id, name) in gone {
            self.emit("peer.down", json!({ "deviceId": id, "name": name }));
        }
        let _ = stale_ms;
    }

    /// Learns or refreshes a peer from a verified announce or info response.
    pub fn note_peer(&self, peer: Peer) {
        let id = peer.device_id.clone();
        if id == self.identity.device_id {
            return;
        }
        if self.is_blocked(&id, &peer.fingerprint) {
            return;
        }
        let existed = {
            let mut peers = lock(&self.peers);
            let existed = peers.contains_key(&id);
            peers.insert(id.clone(), peer.clone());
            existed
        };
        let _ = self.upsert_device(&id, &peer.alias, &peer.device_type, &peer.os, &peer.fingerprint);
        if !existed {
            self.emit("peer.up", json!({ "deviceId": id, "name": peer.alias }));
        }
    }

    pub fn peer_from_info(&self, info: &dg_types::InfoResponse, ip: IpAddr, port: u16) -> Peer {
        Peer {
            device_id: info.device_id.clone(),
            alias: info.alias.clone(),
            device_type: info.device_type.clone(),
            os: info.os.clone(),
            fingerprint: info.fingerprint.clone(),
            ip,
            port,
            last_seen_ms: now_ms(),
            link: "same network".into(),
        }
    }

    pub fn snapshot_settings_json(&self) -> Value {
        self.settings().to_json()
    }

    pub fn socket_addr(&self, ip: IpAddr, port: u16) -> SocketAddr {
        SocketAddr::new(ip, port)
    }
}

impl Peer {
    fn alias_or_name(&self) -> String {
        self.alias.clone()
    }
}

async fn bind_peer(cfg: &Config) -> Result<std::net::TcpListener, String> {
    if cfg.peer_port == 0 {
        let l = std::net::TcpListener::bind(SocketAddr::new(cfg.peer_bind, 0)).map_err(|e| e.to_string())?;
        l.set_nonblocking(true).map_err(|e| e.to_string())?;
        return Ok(l);
    }
    let mut last = String::new();
    for offset in 0..=cfg.peer_port_fallbacks {
        let port = cfg.peer_port.saturating_add(offset);
        match std::net::TcpListener::bind(SocketAddr::new(cfg.peer_bind, port)) {
            Ok(l) => {
                l.set_nonblocking(true).map_err(|e| e.to_string())?;
                return Ok(l);
            }
            Err(e) => last = e.to_string(),
        }
    }
    Err(format!("no free peer port from {}: {last}", cfg.peer_port))
}

/// Best-effort LAN address without sending any packet (UDP connect only sets the route).
pub fn detect_local_ip() -> IpAddr {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0");
    if let Ok(s) = sock {
        if s.connect("239.255.42.99:53318").is_ok() {
            if let Ok(addr) = s.local_addr() {
                if !addr.ip().is_unspecified() {
                    return addr.ip();
                }
            }
        }
    }
    IpAddr::V4(Ipv4Addr::LOCALHOST)
}

/// One pump tick: coalesced progress for sessions and downloads, then peer sweeps (section 3.4).
pub fn pump_once(engine: &Engine) {
    transfer::pump_sessions(engine);
    downloads::pump_downloads(engine);
    engine.sweep_peers();
    crate::staging::sweep(engine);
}

pub fn free_bytes(path: &std::path::Path) -> Option<u64> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        let c = CString::new(path.as_os_str().to_string_lossy().as_bytes()).ok()?;
        // SAFETY: `libc::statvfs` is a plain C struct of integers; all-zero is a valid value.
        let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
        // SAFETY: `c` is a valid NUL-terminated path and `st` is a valid out-pointer.
        let rc = unsafe { libc::statvfs(c.as_ptr(), &mut st) };
        if rc != 0 {
            return None;
        }
        Some((st.f_bavail as u64).saturating_mul(st.f_frsize as u64))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}
