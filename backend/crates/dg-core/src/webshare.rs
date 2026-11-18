//! Web-share (section 4.8): a temporary HTTPS server with a random path, a 6-digit PIN entered
//! on the page, expiry (default 10 minutes), and a single-session lock. Send mode lists the
//! files; the page is served from the same server.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::body::Body;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde_json::{json, Value};
use tokio_util::io::ReaderStream;

use crate::engine::{err, lock, Engine, Res};
use crate::util::{now_ms, random_hex};
use dg_types::ErrorCode;

pub struct ShareHandle {
    pub token: String,
    pub pin: String,
    pub url: String,
    pub expires_ms: i64,
    pub files: Vec<(String, std::path::PathBuf, u64)>,
    pub locked_session: Arc<AtomicBool>,
    pub handle: axum_server::Handle<SocketAddr>,
}

impl ShareHandle {
    pub fn stop(&self) {
        self.handle.graceful_shutdown(Some(Duration::from_secs(1)));
    }
}

/// Wrong PIN guesses allowed before the share refuses every further unlock attempt.
const MAX_PIN_ATTEMPTS: u32 = 5;

#[derive(Clone)]
struct ShareState {
    token: String,
    pin: String,
    expires_ms: i64,
    files: Arc<Vec<(String, std::path::PathBuf, u64)>>,
    session_open: Arc<AtomicBool>,
    failed_pins: Arc<AtomicU32>,
}

fn share_router(st: ShareState) -> Router {
    Router::new()
        .route("/s/{token}", get(page))
        .route("/s/{token}/files", get(list_files))
        .route("/s/{token}/file/{idx}", get(download_file))
        .route("/s/{token}/unlock/{pin}", get(unlock))
        .with_state(st)
}

fn gone() -> Response {
    (StatusCode::NOT_FOUND, "This link is not available.").into_response()
}

async fn page(State(st): State<ShareState>, Path(token): Path<String>) -> Response {
    if token != st.token || now_ms() > st.expires_ms {
        return gone();
    }
    let html = PAGE_HTML.replace("__TOKEN__", &st.token);
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8"), (header::CACHE_CONTROL, "no-store")], html).into_response()
}

async fn unlock(State(st): State<ShareState>, Path((token, pin)): Path<(String, String)>) -> Response {
    if token != st.token || now_ms() > st.expires_ms {
        return gone();
    }
    if st.failed_pins.load(Ordering::SeqCst) >= MAX_PIN_ATTEMPTS {
        return (StatusCode::TOO_MANY_REQUESTS, Json_err("too many wrong codes")).into_response();
    }
    if pin != st.pin {
        st.failed_pins.fetch_add(1, Ordering::SeqCst);
        return (StatusCode::FORBIDDEN, Json_err("wrong code")).into_response();
    }
    if st.session_open.swap(true, Ordering::SeqCst) {
        // Single-session lock: a second browser is refused until the first one finishes.
        return (StatusCode::CONFLICT, Json_err("another browser is using this link")).into_response();
    }
    let list: Vec<Value> = st.files.iter().enumerate().map(|(i, (n, _, s))| json!({ "index": i, "name": n, "size": s })).collect();
    axum::Json(json!({ "files": list })).into_response()
}

#[allow(non_snake_case)]
fn Json_err(msg: &str) -> String {
    format!("{{\"error\":\"{msg}\"}}")
}

async fn list_files(State(st): State<ShareState>, Path(token): Path<String>) -> Response {
    if token != st.token || now_ms() > st.expires_ms || !st.session_open.load(Ordering::SeqCst) {
        return gone();
    }
    let list: Vec<Value> = st.files.iter().enumerate().map(|(i, (n, _, s))| json!({ "index": i, "name": n, "size": s })).collect();
    axum::Json(json!({ "files": list })).into_response()
}

async fn download_file(State(st): State<ShareState>, Path((token, idx)): Path<(String, usize)>) -> Response {
    if token != st.token || now_ms() > st.expires_ms || !st.session_open.load(Ordering::SeqCst) {
        return gone();
    }
    let Some((name, path, _)) = st.files.get(idx).cloned() else {
        return gone();
    };
    // Stream from disk so large files are never held in memory.
    let Ok(file) = tokio::fs::File::open(&path).await else {
        return gone();
    };
    let disp = format!("attachment; filename=\"{}\"", name.replace('"', "_"));
    let size = st.files.get(idx).map(|f| f.2).unwrap_or(0);
    (
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (header::CONTENT_DISPOSITION, disp),
            (header::CONTENT_LENGTH, size.to_string()),
        ],
        Body::from_stream(ReaderStream::new(file)),
    )
        .into_response()
}

const PAGE_HTML: &str = r#"<!doctype html><meta charset="utf-8"><title>DropGrab</title>
<h1>Files shared with you</h1>
<p>Enter the 6-digit code shown on the sending device.</p>
<form onsubmit="event.preventDefault();unlock(this.pin.value)"><input name="pin" inputmode="numeric" maxlength="6" autocomplete="one-time-code"><button>Continue</button></form>
<ul id="files"></ul>
<script>
const token = "__TOKEN__";
async function unlock(pin) {
  const r = await fetch(`/s/${token}/unlock/${pin}`);
  if (!r.ok) { alert("That code is not right."); return; }
  const data = await r.json();
  const ul = document.getElementById("files");
  ul.replaceChildren();  // file names below are set with textContent only
  for (const f of data.files) {
    const li = document.createElement("li");
    const a = document.createElement("a");
    a.href = `/s/${token}/file/${f.index}`;
    a.textContent = `${f.name} (${f.size} bytes)`;
    li.appendChild(a);
    ul.appendChild(li);
  }
}
</script>
"#;

/// `webshare.start {files: [{path, name}]}`: serves the listed files for ten minutes.
pub async fn start(engine: &Engine, params: &Value) -> Res<Value> {
    if lock(&engine.webshare).is_some() {
        return Err(err(ErrorCode::Busy, "a web share is already running"));
    }
    let raw_files = params.get("files").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut files = Vec::new();
    for f in raw_files {
        let path = f.get("path").and_then(Value::as_str).map(std::path::PathBuf::from);
        let id = f.get("id").and_then(Value::as_str).map(str::to_string);
        let (path, name) = match (path, id) {
            (Some(p), _) => {
                let n = f.get("name").and_then(Value::as_str).map(str::to_string).or_else(|| p.file_name().map(|n| n.to_string_lossy().to_string())).unwrap_or_default();
                (p, n)
            }
            (None, Some(id)) => {
                let it = engine.db.item_get(&id).map_err(crate::engine::internal)?.ok_or_else(|| err(ErrorCode::UnknownItem, "no such item"))?;
                (std::path::PathBuf::from(it.path), it.display_name)
            }
            _ => return Err(err(ErrorCode::InvalidParams, "each file needs a path or a library id")),
        };
        let meta = std::fs::metadata(&path).map_err(|_| err(ErrorCode::InvalidParams, format!("cannot read {}", path.display())))?;
        files.push((crate::util::sanitize_filename(&name), path, meta.len()));
    }
    if files.is_empty() {
        return Err(err(ErrorCode::InvalidParams, "no files to share"));
    }
    let token = random_hex(12);
    let pin = format!("{:06}", rand::random::<u32>() % 1_000_000);
    let expires = now_ms() + engine.cfg.webshare_ttl.as_millis() as i64;
    let session_open = Arc::new(AtomicBool::new(false));
    let st = ShareState {
        token: token.clone(),
        pin: pin.clone(),
        expires_ms: expires,
        files: Arc::new(files.clone()),
        session_open: session_open.clone(),
        failed_pins: Arc::new(AtomicU32::new(0)),
    };
    let handle = axum_server::Handle::<SocketAddr>::new();
    let tls = build_tls(&engine.identity.cert_pem, &engine.identity.key_pem).await?;
    let addr: SocketAddr = format!("0.0.0.0:{}", engine.cfg.webshare_port).parse().map_err(crate::engine::internal)?;
    let listener = tokio::net::TcpListener::bind(addr).await;
    let std_listener = match listener {
        Ok(l) => l.into_std().map_err(crate::engine::internal)?,
        Err(_) => std::net::TcpListener::bind("0.0.0.0:0").map_err(crate::engine::internal)?,
    };
    std_listener.set_nonblocking(true).map_err(crate::engine::internal)?;
    let bound = std_listener.local_addr().map_err(crate::engine::internal)?;
    let server_handle = handle.clone();
    let app = share_router(st).into_make_service();
    engine.spawn(async move {
        if let Ok(server) = axum_server::from_tcp_rustls(std_listener, tls) {
            let _ = server.handle(server_handle).serve(app).await;
        }
    });
    let ip = engine.advertise_ip();
    let url = format!("https://{ip}:{}/s/{token}", bound.port());
    let share = ShareHandle {
        token: token.clone(),
        pin: pin.clone(),
        url: url.clone(),
        expires_ms: expires,
        files: files.clone(),
        locked_session: session_open,
        handle,
    };
    let total: u64 = files.iter().map(|f| f.2).sum();
    let listing: Vec<Value> = files.iter().map(|(n, _, s)| json!({ "name": n, "size": s })).collect();
    *lock(&engine.webshare) = Some(share);
    // Expiry timer: stop the server and forget the share (section 4.8).
    // Only this share is stopped when the timer fires; a newer share keeps running.
    let eng = engine.clone();
    let ttl = engine.cfg.webshare_ttl;
    let my_token = token.clone();
    engine.spawn(async move {
        tokio::time::sleep(ttl).await;
        stop_matching(&eng, Some(&my_token));
    });
    engine.emit("webshare.started", json!({ "url": url, "expiresAt": expires }));
    Ok(json!({
        "url": url,
        "pin": pin,
        "files": listing,
        "total": total,
        "expiresAt": expires,
        "fingerprint": crate::util::display_fingerprint(&engine.identity.fingerprint),
    }))
}

/// Stops the running share. With `only_token`, stops it only if it is that share.
fn stop_matching(engine: &Engine, only_token: Option<&str>) {
    let taken = {
        let mut guard = lock(&engine.webshare);
        match (guard.as_ref(), only_token) {
            (None, _) => None,
            (Some(share), Some(t)) if share.token != t => None,
            _ => guard.take(),
        }
    };
    if let Some(share) = taken {
        share.stop();
        engine.emit("webshare.stopped", json!({ "reason": "expired-or-stopped" }));
    }
}

pub fn stop(engine: &Engine) -> Res<Value> {
    let had = lock(&engine.webshare).is_some();
    stop_matching(engine, None);
    Ok(json!({ "stopped": had }))
}

async fn build_tls(cert_pem: &str, key_pem: &str) -> Res<axum_server::tls_rustls::RustlsConfig> {
    crate::identity::install_crypto_provider();
    axum_server::tls_rustls::RustlsConfig::from_pem(cert_pem.as_bytes().to_vec(), key_pem.as_bytes().to_vec())
        .await
        .map_err(crate::engine::internal)
}

impl Engine {
    pub(crate) fn start_peer_server(&self, listener: std::net::TcpListener) -> Result<(), String> {
        let handle = axum_server::Handle::<SocketAddr>::new();
        *lock(&self.peer_server) = Some(handle.clone());
        // Mutual TLS: request client certificates and record them per connection (peer.rs).
        let tls_config = crate::identity::peer_server_config(&self.identity)?;
        let router = crate::peer::router(self.clone());
        self.spawn(async move {
            let tls = axum_server::tls_rustls::RustlsConfig::from_config(tls_config);
            let server = match axum_server::from_tcp(listener) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("peer listener failed: {e}");
                    return;
                }
            };
            let server = server.acceptor(crate::peer::MtlsAcceptor::new(tls));
            if let Err(e) = server
                .handle(handle)
                .serve(router.into_make_service_with_connect_info::<SocketAddr>())
                .await
            {
                tracing::error!("peer server stopped: {e}");
            }
        });
        Ok(())
    }
}
