//! Control API contract (dropgrab-app-design.md section 3.5).
//!
//! Every command and event name in section 3.5 must (a) be listed in `dg_types`, (b) be reachable
//! through the real `Backend::invoke` path, answering with something other than UNKNOWN_METHOD or
//! NOT_IMPLEMENTED, and (c) for events, be emitted by the engine. The HTTP surface (`/rpc`,
//! `/events`) is exercised over a real loopback socket as well.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use dg_core::{api, engine::Config, Backend};
use dg_types::{ErrorCode, COMMANDS, EVENTS};
use serde_json::{json, Value};

/// The §3.5 table, copied from the design document. Kept separately from `dg_types::COMMANDS` so
/// the test catches drift in either place.
const DESIGN_COMMANDS: &[&str] = &[
    "devices.list", "devices.trust", "devices.block", "devices.forget", "devices.set_allowance",
    "visibility.set", "session.send", "session.respond", "session.pause", "session.resume",
    "session.cancel", "session.history", "text.send", "pair.qr_create", "pair.qr_scan",
    "downloads.add", "downloads.probe", "downloads.pause", "downloads.resume", "downloads.cancel",
    "downloads.reorder", "downloads.set_priority", "downloads.list", "extractor.resolve",
    "extractor.update", "extractor.sites", "library.query", "library.get", "library.delete",
    "library.move", "library.duplicates", "remote.request_download", "remote.list_requests",
    "webshare.start", "webshare.stop", "settings.get", "settings.set", "diagnostics.run",
    "logs.export", "backend.info", "backend.shutdown",
];

/// The §3.5 event names.
const DESIGN_EVENTS: &[&str] = &[
    "peer.up", "peer.down", "session.offered", "session.state", "session.progress",
    "session.finished", "download.state", "download.progress", "download.finished",
    "library.changed", "device.changed", "notification.request", "error", "resync",
];

struct TestDir(PathBuf);
impl TestDir {
    fn new(tag: &str) -> TestDir {
        let p = std::env::temp_dir().join(format!("dg-test-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn start(tag: &str) -> (Backend, TestDir) {
    let dir = TestDir::new(tag);
    let mut cfg = Config::desktop(dir.0.join("data"), dir.0.join("downloads"));
    cfg.peer_port = 0;
    cfg.discovery = false;
    cfg.device_name = "Contract Test".into();
    let backend = Backend::start(cfg).await.expect("backend starts");
    (backend, dir)
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|s| s.to_string()).collect()
}

#[test]
fn command_list_matches_design_table() {
    assert_eq!(DESIGN_COMMANDS.len(), 41, "section 3.5 table has 41 commands");
    assert_eq!(set(COMMANDS), set(DESIGN_COMMANDS));
}

#[test]
fn event_list_matches_design() {
    assert_eq!(DESIGN_EVENTS.len(), 14);
    assert_eq!(set(EVENTS), set(DESIGN_EVENTS));
}

/// Parameters that make each command reach its handler. Commands with no entry get `{}`.
fn params_for(method: &str) -> Value {
    match method {
        "devices.trust" | "devices.block" | "devices.forget" | "devices.set_allowance" => {
            json!({ "deviceId": "no-such-device", "allowance": 0 })
        }
        "visibility.set" => json!({ "visibility": "everyone" }),
        "session.respond" => json!({ "sid": "no-such-session", "accept": false }),
        "session.pause" | "session.resume" | "session.cancel" => json!({ "sid": "no-such-session" }),
        "text.send" => json!({ "peer": "no-such-device", "text": "hi" }),
        "pair.qr_scan" => json!({ "uri": "dropgrab://pair?v=1&id=a&fp=b&ip=127.0.0.1&port=1&pin=123456" }),
        "downloads.probe" => json!({ "url": "http://127.0.0.1:1/none.bin" }),
        "downloads.add" => json!({ "url": "http://127.0.0.1:1/none.bin", "kind": "direct" }),
        "downloads.pause" | "downloads.resume" | "downloads.cancel" => json!({ "id": "no-such-job" }),
        "downloads.reorder" => json!({ "order": [] }),
        "downloads.set_priority" => json!({ "id": "no-such-job", "priority": "normal" }),
        "extractor.resolve" => json!({ "url": "http://127.0.0.1:1/page" }),
        "extractor.update" => json!({}),
        "library.get" | "library.delete" => json!({ "id": "no-such-item" }),
        "library.move" => json!({ "id": "no-such-item", "folder": "/tmp" }),
        "remote.request_download" => json!({ "peer": "no-such-device", "url": "https://example.com/a.bin" }),
        "settings.set" => json!({ "theme": "dark" }),
        "webshare.start" => json!({ "files": [] }),
        _ => json!({}),
    }
}

#[tokio::test]
async fn every_command_is_reachable_through_invoke() {
    let (backend, _dir) = start("reach").await;
    let mut failures = Vec::new();
    for &cmd in DESIGN_COMMANDS {
        // `backend.shutdown` flips the stop flag; invoke it last.
        if cmd == "backend.shutdown" {
            continue;
        }
        let res = backend.invoke(cmd, params_for(cmd)).await;
        if let Err(e) = res {
            let code = e.code.as_str();
            if code == ErrorCode::UnknownMethod.as_str() || code == ErrorCode::NotImplemented.as_str() {
                failures.push(format!("{cmd}: {code} ({})", e.message));
            }
        }
    }
    assert!(failures.is_empty(), "commands not reachable: {failures:#?}");
    let r = backend.invoke("backend.shutdown", json!({})).await.expect("shutdown answers");
    assert_eq!(r["stopping"], json!(true));
    backend.stop().await;
}

#[tokio::test]
async fn unknown_method_is_reported_not_silently_ignored() {
    let (backend, _dir) = start("unknown").await;
    let e = backend.invoke("nope.nope", json!({})).await.unwrap_err();
    assert_eq!(e.code, ErrorCode::UnknownMethod.as_str());
    backend.stop().await;
}

#[tokio::test]
async fn every_design_event_is_emitted_somewhere_in_the_engine() {
    // Static check: each §3.5 event name must appear as a string literal in an engine source file
    // (the engine emits them through `emit("<name>", ...)`). Runtime emission of each event is
    // covered by the e2e tests in transfer_e2e.rs and download_e2e.rs for the events they reach.
    let src_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut all = String::new();
    for entry in std::fs::read_dir(&src_dir).unwrap() {
        let p = entry.unwrap().path();
        if p.extension().map(|e| e == "rs").unwrap_or(false) {
            all.push_str(&std::fs::read_to_string(&p).unwrap());
        }
    }
    let missing: Vec<&str> = DESIGN_EVENTS
        .iter()
        .copied()
        .filter(|e| !all.contains(&format!("\"{e}\"")))
        .collect();
    assert!(missing.is_empty(), "events never emitted: {missing:?}");
}

#[tokio::test]
async fn http_rpc_and_sse_over_loopback() {
    let (backend, dir) = start("http").await;
    let token = api::load_or_create_token(&dir.0.join("data")).unwrap();
    let (addr, server) = api::serve(backend.engine.clone(), 0, token.clone()).await.unwrap();
    let client = reqwest::Client::new();
    let url = format!("http://{addr}/rpc");

    // No token: 401.
    let r = client.post(&url).json(&json!({ "id": 1, "method": "backend.info" })).send().await.unwrap();
    assert_eq!(r.status(), 401);

    // Wrong Host header: 403 (DNS-rebinding guard).
    let r = client
        .post(&url)
        .header("Host", "evil.example")
        .bearer_auth(&token)
        .json(&json!({ "id": 2, "method": "backend.info" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);

    // Good request: backend.info answers with the API version and device block.
    let r: Value = client
        .post(&url)
        .bearer_auth(&token)
        .json(&json!({ "id": 3, "method": "backend.info" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(r["id"], json!(3));
    assert_eq!(r["result"]["apiVersion"], json!(dg_types::API_VERSION));
    assert_eq!(r["result"]["device"]["name"], json!("Contract Test"));

    // Unknown method comes back as an error object in the same envelope.
    let r: Value = client
        .post(&url)
        .bearer_auth(&token)
        .json(&json!({ "id": 4, "method": "nope.nope" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(r["error"]["code"], json!("UNKNOWN_METHOD"));

    // A setting change emits an event; the SSE stream replays it from seq 0.
    let _ = client
        .post(&url)
        .bearer_auth(&token)
        .json(&json!({ "id": 5, "method": "settings.set", "params": { "theme": "dark" } }))
        .send()
        .await
        .unwrap();
    let mut resp = client
        .get(format!("http://{addr}/events?after=0"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let mut body = String::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while tokio::time::Instant::now() < deadline && !body.contains("settings.changed") {
        match tokio::time::timeout(Duration::from_millis(500), resp.chunk()).await {
            Ok(Ok(Some(chunk))) => body.push_str(&String::from_utf8_lossy(&chunk)),
            _ => {}
        }
    }
    assert!(body.contains("event: settings.changed"), "SSE body: {body}");
    assert!(body.contains("\"type\":\"settings.changed\""), "SSE body: {body}");

    server.abort();
    backend.stop().await;
}
