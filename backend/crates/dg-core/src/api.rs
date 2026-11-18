//! Control API (dropgrab-app-design.md section 3.5).
//!
//! Transport: loopback HTTP (`/rpc` for commands, `/events` for the SSE stream). Access rules
//! from section 3.5 "Local API security": a per-install bearer token on every request, a `Host`
//! allowlist (DNS-rebinding guard), an `Origin` allowlist, no CORS headers, a request size cap,
//! and a per-second command rate limit.
//!
//! Every method in `dg_types::COMMANDS` is dispatched in `dispatch`. Unknown names are answered
//! with UNKNOWN_METHOD so a client can detect a version mismatch.

use std::net::SocketAddr;
use std::path::Path as FsPath;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use dg_types::{ApiError, ErrorCode, RpcRequest, RpcResponse, API_VERSION, COMMANDS, EVENTS, EXTENSION_COMMANDS};
use futures_util::stream::{self, Stream};
use serde_json::{json, Map, Value};
use std::convert::Infallible;
use tokio::sync::broadcast;

use crate::engine::{err, internal, lock, Engine, Res};
use crate::events::Replay;
use crate::{discovery, downloads, extractor, library, settings::Settings, transfer, webshare};

pub const MAX_REQUEST_BYTES: usize = 64 * 1024;
/// Body cap for `/rpc`. Only `files.stage` may use more than `MAX_REQUEST_BYTES`; its base64
/// payload carries at most `staging::MAX_STAGE_BYTES`, and every other method keeps the 64 KiB
/// limit (checked in `rpc_inner`).
pub const MAX_RPC_BODY_BYTES: usize = crate::staging::MAX_STAGE_BYTES.div_ceil(3) * 4 + MAX_REQUEST_BYTES;
pub const TOKEN_FILE: &str = "api.token";

#[derive(Clone)]
pub struct ApiState {
    pub engine: Engine,
    pub token: String,
    pub allowed_hosts: Vec<String>,
    pub allowed_origins: Vec<String>,
}

pub fn router(state: ApiState) -> Router {
    let rpc = post(rpc)
        .options(preflight)
        .layer(DefaultBodyLimit::max(MAX_RPC_BODY_BYTES));
    Router::new()
        .route("/rpc", rpc)
        .route("/events", get(events).options(preflight))
        .route("/health", get(health))
        .with_state(state)
}

/// Trims whitespace and the trailing slash from a configured origin, so `http://localhost:8080`
/// and `http://localhost:8080/` behave the same. Browsers send the form without a trailing slash.
pub fn normalize_origin(raw: &str) -> String {
    raw.trim().trim_end_matches('/').to_string()
}

/// Loads or creates the per-install token. The file is readable only by the owner.
pub fn load_or_create_token(data_dir: &FsPath) -> std::io::Result<String> {
    std::fs::create_dir_all(data_dir)?;
    let path = data_dir.join(TOKEN_FILE);
    if let Ok(t) = std::fs::read_to_string(&path) {
        let t = t.trim().to_string();
        if t.len() >= 32 {
            return Ok(t);
        }
    }
    let t = crate::util::random_hex(32);
    std::fs::write(&path, &t)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(t)
}

fn check_headers(st: &ApiState, headers: &HeaderMap, need_token: bool) -> Result<(), (StatusCode, &'static str)> {
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok()).unwrap_or("");
    let host_name = host.split(':').next().unwrap_or("");
    if !st.allowed_hosts.iter().any(|h| h == host_name) {
        return Err((StatusCode::FORBIDDEN, "host not allowed"));
    }
    if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        let sent = normalize_origin(origin);
        if !st.allowed_origins.iter().any(|o| o == &sent) {
            return Err((StatusCode::FORBIDDEN, "origin not allowed"));
        }
    }
    if need_token {
        let ok = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|t| constant_time_eq(t.as_bytes(), st.token.as_bytes()))
            .unwrap_or(false);
        if !ok {
            return Err((StatusCode::UNAUTHORIZED, "missing or wrong token"));
        }
    }
    Ok(())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

async fn health() -> &'static str {
    "ok"
}

/// The request's `Origin` if it is on the allowlist. `None` means either no `Origin` header
/// (non-browser caller) or an origin that `check_headers` already rejected.
fn cors_origin(st: &ApiState, headers: &HeaderMap) -> Option<String> {
    let origin = headers.get(header::ORIGIN)?.to_str().ok()?;
    let origin = normalize_origin(origin);
    st.allowed_origins.iter().find(|o| **o == origin).cloned()
}

/// Adds CORS headers for an allowed origin. Deny-all (the default) sends none, as before.
fn with_cors(mut resp: Response, origin: Option<&str>) -> Response {
    if let Some(o) = origin {
        if let Ok(v) = axum::http::HeaderValue::from_str(o) {
            let h = resp.headers_mut();
            h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, v);
            h.insert(header::VARY, axum::http::HeaderValue::from_static("Origin"));
        }
    }
    resp
}

/// CORS preflight. A browser never sends the bearer token on `OPTIONS`, so only the `Host` and
/// `Origin` policy is checked here; the actual request still needs the token.
async fn preflight(State(st): State<ApiState>, headers: HeaderMap) -> Response {
    if let Err((code, msg)) = check_headers(&st, &headers, false) {
        return (code, msg).into_response();
    }
    let mut resp = StatusCode::NO_CONTENT.into_response();
    let h = resp.headers_mut();
    h.insert(header::ACCESS_CONTROL_ALLOW_METHODS, axum::http::HeaderValue::from_static("POST, GET, OPTIONS"));
    h.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, axum::http::HeaderValue::from_static("authorization, content-type, last-event-id"));
    h.insert(header::ACCESS_CONTROL_MAX_AGE, axum::http::HeaderValue::from_static("600"));
    with_cors(resp, cors_origin(&st, &headers).as_deref())
}

async fn rpc(State(st): State<ApiState>, headers: HeaderMap, body: Bytes) -> Response {
    let origin = cors_origin(&st, &headers);
    let resp = rpc_inner(&st, &headers, &body).await;
    with_cors(resp, origin.as_deref())
}

async fn rpc_inner(st: &ApiState, headers: &HeaderMap, body: &[u8]) -> Response {
    if let Err((code, msg)) = check_headers(st, headers, true) {
        return (code, msg).into_response();
    }
    let req: RpcRequest = match serde_json::from_slice(body) {
        Ok(r) => r,
        Err(e) => {
            let resp = RpcResponse {
                id: Value::Null,
                result: None,
                error: Some(ApiError::new(ErrorCode::InvalidParams, format!("malformed request: {e}"))),
            };
            return axum::Json(resp).into_response();
        }
    };
    // The raised body cap exists only for `files.stage`; everything else keeps 64 KiB.
    if body.len() > MAX_REQUEST_BYTES && req.method != "files.stage" {
        return (StatusCode::PAYLOAD_TOO_LARGE, "request too large").into_response();
    }
    let id = req.id.clone();
    let resp = match dispatch(&st.engine, &req.method, req.params).await {
        Ok(v) => RpcResponse { id, result: Some(v), error: None },
        Err(e) => RpcResponse { id, result: None, error: Some(e) },
    };
    axum::Json(resp).into_response()
}

#[derive(serde::Deserialize)]
pub struct EventsQuery {
    #[serde(default)]
    pub after: u64,
}

/// Server-sent events. Clients send `?after=<seq>` (or `Last-Event-ID`) to replay missed events.
/// When the gap is larger than the replay window, a `resync` event tells the client to reload
/// state through query commands (section 3.5).
async fn events(
    State(st): State<ApiState>,
    headers: HeaderMap,
    Query(q): Query<EventsQuery>,
) -> Response {
    let origin = cors_origin(&st, &headers);
    match events_inner(st, headers, q).await {
        Ok(sse) => with_cors(sse.into_response(), origin.as_deref()),
        Err(resp) => resp,
    }
}

async fn events_inner(
    st: ApiState,
    headers: HeaderMap,
    q: EventsQuery,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, Response> {
    if let Err((code, msg)) = check_headers(&st, &headers, true) {
        return Err((code, msg).into_response());
    }
    let after = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(q.after);
    let engine = st.engine.clone();
    // Subscribe before reading the backlog so no event falls between the two.
    let mut rx = engine.bus.subscribe();
    let backlog = initial_events(&engine, after);
    let mut seen = after;
    if let Some(last) = backlog.last() {
        seen = seen.max(last.seq);
    }
    let (tx, rx_out) = tokio::sync::mpsc::channel::<Result<SseEvent, Infallible>>(256);
    engine.spawn(async move {
        for ev in backlog {
            if tx.send(Ok(to_sse(&ev))).await.is_err() {
                return;
            }
        }
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    if ev.seq <= seen {
                        continue;
                    }
                    seen = ev.seq;
                    if tx.send(Ok(to_sse(&ev))).await.is_err() {
                        return;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let last = dg_types::Event { seq: seen, kind: "resync".into(), data: json!({ "reason": "lagged" }) };
                    if tx.send(Ok(to_sse(&last))).await.is_err() {
                        return;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    });
    let out = stream::unfold(rx_out, |mut rx| async move { rx.recv().await.map(|item| (item, rx)) });
    Ok(Sse::new(out).keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(15))))
}

fn initial_events(engine: &Engine, after: u64) -> Vec<dg_types::Event> {
    match engine.bus.replay(after) {
        Replay::Events(evs) => evs,
        Replay::Resync => vec![dg_types::Event { seq: engine.bus.last_seq(), kind: "resync".into(), data: json!({ "reason": "window" }) }],
    }
}

fn to_sse(ev: &dg_types::Event) -> SseEvent {
    let body = json!({ "seq": ev.seq, "type": ev.kind, "data": ev.data });
    SseEvent::default().id(ev.seq.to_string()).event(ev.kind.clone()).data(body.to_string())
}

/* ---------- dispatch ---------- */

fn params_obj(v: &Value) -> Res<Map<String, Value>> {
    match v {
        Value::Null => Ok(Map::new()),
        Value::Object(m) => Ok(m.clone()),
        _ => Err(err(ErrorCode::InvalidParams, "params must be an object")),
    }
}

fn str_param<'a>(p: &'a Value, key: &str) -> Res<&'a str> {
    p.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| err(ErrorCode::InvalidParams, format!("{key} is required")))
}

/// Dispatches one command. Every name in `COMMANDS` has an arm here; the contract test checks this.
pub async fn dispatch(engine: &Engine, method: &str, params: Value) -> Res<Value> {
    engine.rate_check()?;
    if !COMMANDS.contains(&method) && !EXTENSION_COMMANDS.contains(&method) {
        return Err(err(ErrorCode::UnknownMethod, format!("unknown method {method}")));
    }
    let p = params_obj(&params).map(Value::Object)?;
    match method {
        /* devices */
        "devices.list" => {
            let rows = engine.db.devices_all().map_err(internal)?;
            let devices: Vec<Value> = rows.iter().map(|r| engine.device_json(r)).collect();
            Ok(json!({ "devices": devices }))
        }
        "devices.trust" => {
            let id = str_param(&p, "deviceId")?;
            let row = engine.device_row(id)?.ok_or_else(|| err(ErrorCode::UnknownDevice, "unknown device"))?;
            engine.db.device_set_trust(id, "trusted", None).map_err(internal)?;
            engine.emit("device.changed", json!({ "deviceId": id, "trust": "trusted" }));
            Ok(json!({ "deviceId": id, "trust": "trusted", "fingerprint": row.fingerprint }))
        }
        "devices.block" => {
            let id = str_param(&p, "deviceId")?;
            engine.device_row(id)?.ok_or_else(|| err(ErrorCode::UnknownDevice, "unknown device"))?;
            engine.db.device_set_trust(id, "blocked", None).map_err(internal)?;
            lock(&engine.peers).remove(id);
            engine.emit("device.changed", json!({ "deviceId": id, "trust": "blocked" }));
            Ok(json!({ "deviceId": id, "trust": "blocked" }))
        }
        "devices.forget" => {
            let id = str_param(&p, "deviceId")?;
            engine.device_row(id)?.ok_or_else(|| err(ErrorCode::UnknownDevice, "unknown device"))?;
            engine.db.device_set_trust(id, "seen", Some(false)).map_err(internal)?;
            engine.emit("device.changed", json!({ "deviceId": id, "trust": "seen" }));
            Ok(json!({ "deviceId": id, "trust": "seen" }))
        }
        "devices.set_allowance" => {
            let id = str_param(&p, "deviceId")?;
            engine.device_row(id)?.ok_or_else(|| err(ErrorCode::UnknownDevice, "unknown device"))?;
            let auto = p.get("autoAccept").and_then(Value::as_bool);
            let remote = p.get("allowRemote").and_then(Value::as_bool);
            engine.db.device_set_allowance(id, auto, remote).map_err(internal)?;
            engine.emit("device.changed", json!({ "deviceId": id }));
            let row = engine.device_row(id)?.ok_or_else(|| err(ErrorCode::UnknownDevice, "unknown device"))?;
            Ok(json!({ "deviceId": id, "autoAccept": row.auto_accept, "allowRemote": row.allow_remote_download }))
        }
        "visibility.set" => {
            let v = str_param(&p, "visibility")?.to_string();
            let mut patch = Map::new();
            patch.insert("visibility".into(), Value::String(v.clone()));
            let changed = lock(&engine.settings).apply(&patch).map_err(|m| err(ErrorCode::InvalidParams, m))?;
            engine.persist_settings(&changed)?;
            engine.emit("settings.changed", json!({ "visibility": v }));
            Ok(json!({ "visibility": v }))
        }

        /* transfer */
        "session.send" | "transfer.start" => {
            let peer = str_param(&p, "peer")?.to_string();
            let items = p.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
            let sid = transfer::start_send(engine.clone(), &peer, items).await?;
            Ok(json!({ "sid": sid, "state": "offered" }))
        }
        "session.respond" => {
            let sid = str_param(&p, "sid")?;
            let accept = p.get("accept").and_then(Value::as_bool).ok_or_else(|| err(ErrorCode::InvalidParams, "accept is required"))?;
            let trust = p.get("trust").and_then(Value::as_bool).unwrap_or(false);
            transfer::respond(engine, sid, accept, trust).await
        }
        "session.pause" => transfer::control(engine, p.get("sid").and_then(Value::as_str), "pause").await,
        "session.resume" => transfer::control(engine, p.get("sid").and_then(Value::as_str), "resume").await,
        "session.cancel" => transfer::control(engine, p.get("sid").and_then(Value::as_str), "cancel").await,
        "session.history" => transfer::history(engine),
        "session.list" => Ok(transfer::list_active(engine)),
        "text.send" => {
            let peer_id = str_param(&p, "peer")?;
            let text = str_param(&p, "text")?;
            if text.len() > crate::peer::MAX_TEXT {
                return Err(err(ErrorCode::PayloadTooLarge, "text is larger than 64 KiB"));
            }
            let peer = engine.peer_for(peer_id).ok_or_else(|| err(ErrorCode::NetUnreachable, "device is not reachable"))?;
            let client = crate::identity::mtls_client(&engine.identity, &peer.fingerprint, std::time::Duration::from_secs(15)).map_err(internal)?;
            let url = format!("https://{}:{}/api/v1/text", peer.ip, peer.port);
            let body = dg_types::TextRequest { sender: engine.self_sender(), text: text.to_string() };
            let resp = client.post(url).json(&body).send().await.map_err(|_| err(ErrorCode::NetUnreachable, "could not send the text"))?;
            if !resp.status().is_success() {
                return Err(err(ErrorCode::Forbidden, "the device did not accept the text"));
            }
            engine.emit("notification.request", json!({ "kind": "text-sent", "title": format!("Sent the link to {}", peer.alias), "body": text.chars().take(64).collect::<String>() }));
            Ok(json!({ "sent": true }))
        }
        "pair.qr_create" => discovery::qr_create(engine),
        "pair.qr_scan" => discovery::qr_scan(engine, &p).await,

        /* staged browser uploads */
        "files.stage" => crate::staging::stage(engine, &p).await,

        /* downloads */
        "downloads.add" => downloads::add(engine, &p).await,
        "downloads.probe" => {
            let url = str_param(&p, "url")?;
            downloads::probe(engine, url).await
        }
        "downloads.pause" => downloads::pause(engine, str_param(&p, "id")?),
        "downloads.resume" => downloads::resume(engine, str_param(&p, "id")?),
        "downloads.cancel" => downloads::cancel(engine, str_param(&p, "id")?),
        "downloads.reorder" => downloads::reorder(engine, &p),
        "downloads.set_priority" => downloads::set_priority(engine, &p),
        "downloads.list" => Ok(downloads::list(engine)),

        /* media */
        "extractor.resolve" => {
            let url = str_param(&p, "url")?;
            let r = extractor::resolve(engine, url).await?;
            Ok(json!({ "title": r.title, "uploader": r.uploader, "duration": r.duration, "formats": r.formats, "drm": false }))
        }
        "extractor.update" => Ok(extractor::update_status()),
        "extractor.sites" => Ok(extractor::list_sites()),

        /* library */
        "library.query" => library::query(engine, &p),
        "library.get" => library::get(engine, &p),
        "library.delete" => library::delete(engine, &p),
        "library.move" => library::move_item(engine, &p),
        "library.duplicates" => library::duplicates(engine),

        /* remote */
        "remote.request_download" => {
            // Sends from this device to a trusted peer: the peer must allow remote downloads.
            let peer_id = p.get("deviceId").and_then(Value::as_str).ok_or_else(|| err(ErrorCode::InvalidParams, "deviceId is required"))?;
            let url = str_param(&p, "url")?;
            let peer = engine.peer_for(peer_id).ok_or_else(|| err(ErrorCode::NetUnreachable, "device is not reachable"))?;
            let client = crate::identity::mtls_client(&engine.identity, &peer.fingerprint, std::time::Duration::from_secs(15)).map_err(internal)?;
            let body = dg_types::DownloadRequest { sender: engine.self_sender(), url: url.to_string(), name: None };
            let resp = client
                .post(format!("https://{}:{}/api/v1/download-request", peer.ip, peer.port))
                .json(&body)
                .send()
                .await
                .map_err(|_| err(ErrorCode::NetUnreachable, "could not reach the device"))?;
            if !resp.status().is_success() {
                return Err(err(ErrorCode::NotAllowed, "the device does not accept remote downloads from this device"));
            }
            Ok(json!({ "queued": true, "deviceId": peer_id }))
        }
        "remote.list_requests" => {
            let reqs = lock(&engine.remote_requests);
            let rows: Vec<Value> = reqs
                .iter()
                .map(|r| json!({ "id": r.id, "from": r.from_device, "fromName": r.from_name, "url": r.url, "jobId": r.job_id, "at": r.at_ms }))
                .collect();
            Ok(json!({ "requests": rows }))
        }

        /* web share */
        "webshare.start" => webshare::start(engine, &p).await,
        "webshare.stop" => webshare::stop(engine),

        /* system */
        "settings.get" => Ok(json!({ "settings": engine.snapshot_settings_json() })),
        "settings.set" => {
            let patch = params_obj(&p)?;
            let changed = lock(&engine.settings).apply(&patch).map_err(|m| err(ErrorCode::InvalidParams, m))?;
            engine.persist_settings(&changed)?;
            if !changed.is_empty() {
                engine.emit("settings.changed", Value::Object(patch.clone()));
            }
            Ok(json!({ "settings": engine.snapshot_settings_json(), "changed": changed }))
        }
        "diagnostics.run" => Ok(json!({
            "multicastJoined": engine.multicast_joined.load(std::sync::atomic::Ordering::SeqCst),
            "portBound": engine.peer_port,
            "interfaces": [format!("{} {}", "lan", engine.advertise_ip())],
            "peers": lock(&engine.peers).len(),
            "lastSeq": engine.bus.last_seq(),
        })),
        "logs.export" => {
            let path = engine.cfg.data_dir.join("logs-export.txt");
            std::fs::write(&path, format!("DropGrab backend {} (API {})\n", env!("CARGO_PKG_VERSION"), API_VERSION)).map_err(internal)?;
            Ok(json!({ "path": path.display().to_string() }))
        }
        "backend.info" => {
            let s: Settings = engine.settings();
            Ok(json!({
                "apiVersion": API_VERSION,
                "version": format!("dropgrabd {}", env!("CARGO_PKG_VERSION")),
                "mode": "desktop daemon",
                "protocolVersion": dg_types::PROTOCOL_VERSION,
                "port": engine.peer_port,
                "device": {
                    "id": engine.identity.device_id,
                    "name": s.device_name,
                    "type": engine.cfg.device_type,
                    "fingerprint": crate::util::display_fingerprint(&engine.identity.fingerprint),
                    "visibility": s.visibility,
                },
                "events": EVENTS,
                "uptimeMs": crate::util::now_ms() - engine.started_ms,
            }))
        }
        "backend.shutdown" => {
            engine.shutdown.notify_waiters();
            engine.stopping.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(json!({ "stopping": true }))
        }
        _ => Err(err(ErrorCode::NotImplemented, format!("{method} is not implemented"))),
    }
}

/// Peer-facing address used by tests to find the bound peer port.
pub fn peer_addr(engine: &Engine) -> SocketAddr {
    SocketAddr::new(engine.advertise_ip(), engine.peer_port)
}

/// Starts the loopback control server. Binds 127.0.0.1 only (section 3.5: never the LAN).
pub async fn serve(engine: Engine, port: u16, token: String) -> Result<(SocketAddr, tokio::task::JoinHandle<()>), String> {
    serve_with_origins(engine, port, token, Vec::new()).await
}

/// `serve` plus a browser `Origin` allowlist. Empty (the default) rejects every browser origin;
/// a configured origin such as `http://localhost:8080` may call `/rpc` and `/events` with a valid
/// token and receives the matching CORS headers. Requests without an `Origin` header are
/// unaffected and still need only the token.
pub async fn serve_with_origins(
    engine: Engine,
    port: u16,
    token: String,
    allowed_origins: Vec<String>,
) -> Result<(SocketAddr, tokio::task::JoinHandle<()>), String> {
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))).await.map_err(|e| e.to_string())?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?;
    let state = ApiState {
        engine,
        token,
        allowed_hosts: vec!["127.0.0.1".into(), "localhost".into()],
        allowed_origins: allowed_origins.iter().map(|o| normalize_origin(o)).filter(|o| !o.is_empty()).collect(),
    };
    let app = router(state);
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok((addr, handle))
}
