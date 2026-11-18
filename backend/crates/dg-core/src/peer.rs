//! Peer HTTPS server (section 4.4): the endpoints other DropGrab devices call. TLS uses the
//! device's self-signed certificate; clients pin its fingerprint. Callers also present their
//! own certificate (mutual TLS). Every handler that trusts a claimed sender fingerprint checks
//! it against the certificate the peer presented on this connection (`PeerCert`).

use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::body::Bytes;
use axum::extract::{ConnectInfo, DefaultBodyLimit, Extension, Path, Query, State};
use axum::http::{header, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use axum_server::accept::Accept;
use axum_server::tls_rustls::{RustlsAcceptor, RustlsConfig};
use dg_types::{CompleteRequest, DownloadRequest, OfferRequest, PairRequest, TextRequest};
use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::server::TlsStream;
use tower_service::Service;

use crate::engine::{lock, Engine};
use crate::identity::{self, PeerCert};
use crate::{discovery, transfer};

/// Accepts TLS (requesting, not requiring, a client certificate) and records the fingerprint of
/// the certificate the peer presented, so handlers can compare it with the claimed sender.
#[derive(Clone)]
pub struct MtlsAcceptor {
    inner: RustlsAcceptor,
}

impl MtlsAcceptor {
    pub fn new(config: RustlsConfig) -> MtlsAcceptor {
        MtlsAcceptor { inner: RustlsAcceptor::new(config) }
    }
}

impl<I, S> Accept<I, S> for MtlsAcceptor
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    S: Send + 'static,
{
    type Stream = TlsStream<I>;
    type Service = WithPeerCert<S>;
    type Future = Pin<Box<dyn Future<Output = io::Result<(Self::Stream, Self::Service)>> + Send>>;

    fn accept(&self, stream: I, service: S) -> Self::Future {
        let fut = self.inner.accept(stream, service);
        Box::pin(async move {
            let (tls, service) = fut.await?;
            let fp = tls
                .get_ref()
                .1
                .peer_certificates()
                .and_then(|certs| certs.first())
                .map(|c| identity::fingerprint_of(c.as_ref()));
            Ok((tls, WithPeerCert { inner: service, cert: PeerCert(fp) }))
        })
    }
}

/// Adds the connection's `PeerCert` to every request's extensions.
#[derive(Clone)]
pub struct WithPeerCert<S> {
    inner: S,
    cert: PeerCert,
}

impl<S, B> Service<Request<B>> for WithPeerCert<S>
where
    S: Service<Request<B>>,
{
    type Response = <S as Service<Request<B>>>::Response;
    type Error = <S as Service<Request<B>>>::Error;
    type Future = <S as Service<Request<B>>>::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), <S as Service<Request<B>>>::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<B>) -> <S as Service<Request<B>>>::Future {
        req.extensions_mut().insert(self.cert.clone());
        self.inner.call(req)
    }
}

/// 403 unless the presented certificate's fingerprint equals `claimed`.
fn require_sender(cert: &PeerCert, claimed: &str) -> Result<(), Response> {
    identity::check_claimed_fingerprint(cert, claimed).map_err(|m| json_err(403, m))
}

/// For a session route: if the session exists, the caller must present the certificate of the
/// peer that the session belongs to. Unknown sessions fall through to the handler's own 404.
fn require_session_peer(engine: &Engine, cert: &PeerCert, sid: &str) -> Result<(), Response> {
    match transfer::session_peer_fp(engine, sid) {
        Some(fp) => require_sender(cert, &fp),
        None => Ok(()),
    }
}

/// Largest accepted text payload (section 4.9: 64 KiB).
pub const MAX_TEXT: usize = 64 * 1024;
/// Largest single PUT body. The sender uses at most `chunkSize` (4 MiB by default).
pub const MAX_CHUNK: usize = 8 * 1024 * 1024;

#[derive(Clone)]
pub struct PeerState {
    pub engine: Engine,
}

pub fn router(engine: Engine) -> Router {
    let state = PeerState { engine };
    Router::new()
        .route("/api/v1/info", get(info))
        .route("/api/v1/announce", post(announce))
        .route("/api/v1/session/offer", post(offer))
        .route("/api/v1/session/{sid}/file/{fid}", put(upload).layer(DefaultBodyLimit::max(MAX_CHUNK + 1024)))
        .route("/api/v1/session/{sid}/file/{fid}/status", get(status))
        .route("/api/v1/session/{sid}/file/{fid}/complete", post(complete))
        .route("/api/v1/session/{sid}/cancel", post(cancel))
        .route("/api/v1/text", post(text).layer(DefaultBodyLimit::max(MAX_TEXT + 4096)))
        .route("/api/v1/download-request", post(download_request))
        .route("/api/v1/pair", post(pair))
        .with_state(state)
}

fn json_err(code: u16, msg: impl Into<String>) -> Response {
    let status = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(json!({ "error": msg.into() }))).into_response()
}

async fn info(State(s): State<PeerState>) -> Json<dg_types::InfoResponse> {
    Json(s.engine.info())
}

async fn announce(
    State(s): State<PeerState>,
    Extension(cert): Extension<PeerCert>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(msg): Json<dg_types::Announce>,
) -> Response {
    if msg.device_id == s.engine.identity.device_id {
        return StatusCode::NO_CONTENT.into_response();
    }
    if let Err(e) = require_sender(&cert, &msg.fingerprint) {
        return e;
    }
    if s.engine.visibility() != "hidden" {
        s.engine.note_peer(crate::engine::Peer {
            device_id: msg.device_id.clone(),
            alias: msg.alias.clone(),
            device_type: msg.device_type.clone(),
            os: String::new(),
            fingerprint: msg.fingerprint.clone(),
            ip: addr.ip(),
            port: msg.port,
            last_seen_ms: crate::util::now_ms(),
            link: "same network".into(),
        });
    }
    Json(discovery::announce_for(&s.engine, false)).into_response()
}

#[axum::debug_handler]
async fn offer(
    State(s): State<PeerState>,
    Extension(cert): Extension<PeerCert>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<OfferRequest>,
) -> Response {
    // Auto-accept and trust decisions use `sender.fingerprint`; it must be the certificate
    // this peer proved it holds, not a value it merely claims (section 4.4, mutual TLS).
    if let Err(e) = require_sender(&cert, &req.sender.fingerprint) {
        return e;
    }
    match transfer::handle_offer(s.engine.clone(), req, addr.ip(), addr.port()).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => {
            let code = match e.code.as_str() {
                "TLS_MISMATCH" => 403,
                "FORBIDDEN" => 403,
                "BUSY" => 409,
                _ => 400,
            };
            json_err(code, e.message)
        }
    }
}

fn token_of(q: &HashMap<String, String>) -> String {
    q.get("token").cloned().unwrap_or_default()
}

fn parse_range(headers: &axum::http::HeaderMap) -> Option<(u64, u64, u64)> {
    headers
        .get(header::CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(transfer::parse_content_range)
}

async fn upload(
    State(s): State<PeerState>,
    Extension(cert): Extension<PeerCert>,
    Path((sid, fid)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(e) = require_session_peer(&s.engine, &cert, &sid) {
        return e;
    }
    let Some((start, end, total)) = parse_range(&headers) else {
        return json_err(400, "Content-Range is required");
    };
    match transfer::receive_range(s.engine.clone(), &sid, &fid, &token_of(&q), start, end, total, body).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err((code, msg)) => json_err(code, msg),
    }
}

async fn status(
    State(s): State<PeerState>,
    Extension(cert): Extension<PeerCert>,
    Path((sid, fid)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    if let Err(e) = require_session_peer(&s.engine, &cert, &sid) {
        return e;
    }
    match transfer::status_for(&s.engine, &sid, &fid, &token_of(&q)) {
        Ok(st) => Json(st).into_response(),
        Err((code, msg)) => json_err(code, msg),
    }
}

async fn complete(
    State(s): State<PeerState>,
    Extension(cert): Extension<PeerCert>,
    Path((sid, fid)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
    Json(body): Json<CompleteRequest>,
) -> Response {
    if let Err(e) = require_session_peer(&s.engine, &cert, &sid) {
        return e;
    }
    match transfer::complete_file(s.engine.clone(), &sid, &fid, &token_of(&q), Some(body.sha256)).await {
        Ok(v) => Json(v).into_response(),
        Err((code, msg)) => json_err(code, msg),
    }
}

async fn cancel(State(s): State<PeerState>, Extension(cert): Extension<PeerCert>, Path(sid): Path<String>) -> Response {
    if let Err(e) = require_session_peer(&s.engine, &cert, &sid) {
        return e;
    }
    match transfer::remote_cancel(&s.engine, &sid) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err((code, msg)) => json_err(code, msg),
    }
}

async fn text(State(s): State<PeerState>, Extension(cert): Extension<PeerCert>, Json(req): Json<TextRequest>) -> Response {
    if let Err(e) = require_sender(&cert, &req.sender.fingerprint) {
        return e;
    }
    if req.text.len() > MAX_TEXT {
        return json_err(413, "text is larger than 64 KiB");
    }
    if s.engine.is_blocked(&req.sender.device_id, &req.sender.fingerprint) {
        return json_err(403, "blocked");
    }
    s.engine.emit(
        "text.received",
        json!({ "from": req.sender.device_id, "fromName": req.sender.alias, "text": req.text }),
    );
    s.engine.emit(
        "notification.request",
        json!({ "kind": "text", "title": format!("Text from {}", req.sender.alias), "body": req.text.chars().take(64).collect::<String>() }),
    );
    StatusCode::NO_CONTENT.into_response()
}

async fn download_request(State(s): State<PeerState>, Extension(cert): Extension<PeerCert>, Json(req): Json<DownloadRequest>) -> Response {
    // Section 6.3: only trusted senders, only when this device allows remote requests.
    if let Err(e) = require_sender(&cert, &req.sender.fingerprint) {
        return e;
    }
    let Ok(Some(row)) = s.engine.db.device_get(&req.sender.device_id) else {
        return json_err(403, "unknown device");
    };
    if row.trust != "trusted" || row.fingerprint != req.sender.fingerprint || !row.allow_remote_download {
        return json_err(403, "remote download is not allowed for this device");
    }
    if !(req.url.starts_with("https://") || req.url.starts_with("http://")) {
        return json_err(400, "the link must be http or https");
    }
    let id = uuid::Uuid::new_v4().to_string();
    let params = json!({ "url": req.url, "kind": "media", "options": {}, "name": req.name.unwrap_or_default() });
    match crate::downloads::add(&s.engine, &params).await {
        Ok(v) => {
            let job_id = v.get("id").and_then(Value::as_str).map(str::to_string);
            lock(&s.engine.remote_requests).push(crate::engine::RemoteRequest {
                id: id.clone(),
                from_device: req.sender.device_id.clone(),
                from_name: req.sender.alias.clone(),
                url: params["url"].as_str().unwrap_or("").to_string(),
                job_id,
                at_ms: crate::util::now_ms(),
            });
            s.engine.emit(
                "notification.request",
                json!({ "kind": "remote-request", "title": format!("{} asked for a download", req.sender.alias), "body": "Added to Downloads" }),
            );
            (StatusCode::ACCEPTED, Json(json!({ "queued": true, "requestId": id }))).into_response()
        }
        Err(e) => json_err(400, e.message),
    }
}

async fn pair(State(s): State<PeerState>, Extension(cert): Extension<PeerCert>, Json(req): Json<PairRequest>) -> Response {
    if let Err(e) = require_sender(&cert, &req.sender.fingerprint) {
        return e;
    }
    match discovery::handle_pair(&s.engine, req) {
        Ok(resp) => Json(resp).into_response(),
        Err((code, msg)) => json_err(code, msg),
    }
}

