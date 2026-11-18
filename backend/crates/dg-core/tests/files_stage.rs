//! `files.stage` (browser upload staging) and staged-file sending end to end.
//!
//! Covered here:
//! * stage a small file, then send it with `transfer.start` using `{ "stagedId": ... }` and
//!   verify the bytes arrive and the staged copy is removed when the session ends;
//! * the same file sent by the returned `path`;
//! * name traversal (`../../evil.txt`, `a/b.txt`) rejected with INVALID_PARAMS and nothing
//!   written outside the staging folder;
//! * oversize content rejected with PAYLOAD_TOO_LARGE;
//! * over HTTP: `/rpc` accepts a body larger than 64 KiB for `files.stage` only.

use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use base64::Engine as _;
use dg_core::{api, engine::Config, Backend};
use dg_types::ErrorCode;
use serde_json::{json, Value};

struct TestDir(PathBuf);
impl TestDir {
    fn new(tag: &str) -> TestDir {
        let p = std::env::temp_dir().join(format!("dg-stage-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn node(tag: &str, name: &str, dir: &TestDir) -> Backend {
    let mut cfg = Config::desktop(dir.0.join(format!("{tag}-data")), dir.0.join(format!("{tag}-dl")));
    cfg.peer_port = 0;
    cfg.discovery = false;
    cfg.device_name = name.into();
    cfg.advertise_ip = Some(IpAddr::V4(Ipv4Addr::LOCALHOST));
    cfg.chunk_size = 64 * 1024;
    Backend::start(cfg).await.expect("engine starts")
}

fn introduce(a: &Backend, b: &Backend) {
    let info = b.engine.info();
    let peer = a.engine.peer_from_info(&info, IpAddr::V4(Ipv4Addr::LOCALHOST), b.engine.peer_port);
    a.engine.note_peer(peer);
}

async fn wait_state(b: &Backend, sid: &str, want: &str, secs: u64) -> Option<String> {
    let until = Instant::now() + Duration::from_secs(secs);
    let mut last = None;
    while Instant::now() < until {
        let v = b.invoke("session.history", json!({})).await.ok()?;
        last = v["sessions"]
            .as_array()?
            .iter()
            .find(|s| s["sid"].as_str() == Some(sid))
            .and_then(|s| s["state"].as_str().map(str::to_string));
        if last.as_deref() == Some(want) {
            return last;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    last
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn data_dir(dir: &TestDir, tag: &str) -> PathBuf {
    dir.0.join(format!("{tag}-data"))
}

#[tokio::test]
async fn stage_then_send_round_trip_by_id_and_by_path() {
    let dir = TestDir::new("roundtrip");
    let alice = node("a", "Alice Laptop", &dir).await;
    let bob = node("b", "Bob Desktop", &dir).await;
    introduce(&alice, &bob);

    let payload: Vec<u8> = (0..200 * 1024u32).map(|i| (i.wrapping_mul(2654435761) >> 11) as u8).collect();

    // 1. Stage the bytes the way the web client does.
    let staged = alice
        .invoke("files.stage", json!({ "name": "photo.bin", "content": b64(&payload) }))
        .await
        .expect("files.stage accepted");
    let id = staged["id"].as_str().expect("id").to_string();
    let path = staged["path"].as_str().expect("path").to_string();
    assert!(id.starts_with("stg_"), "id shape: {id}");
    assert_eq!(staged["name"], json!("photo.bin"));
    assert_eq!(staged["size"], json!(payload.len()));
    assert_eq!(staged["path"], json!(path));
    let staged_path = PathBuf::from(&path);
    assert!(staged_path.starts_with(data_dir(&dir, "a").join("staging")), "path must be under <data-dir>/staging: {path}");
    assert_eq!(std::fs::read(&staged_path).expect("staged file on disk"), payload);
    // The name is the last segment, directly under the random id folder.
    assert_eq!(staged_path.file_name().unwrap(), "photo.bin");
    assert!(staged_path.parent().unwrap().file_name().unwrap().to_string_lossy().starts_with("stg_"));

    // 2. Send it with transfer.start and the staged id.
    let bob_id = bob.engine.identity.device_id.clone();
    let started = alice
        .invoke("transfer.start", json!({ "peer": bob_id, "items": [{ "stagedId": id }] }))
        .await
        .expect("transfer.start accepted a staged id");
    let sid = started["sid"].as_str().expect("sid").to_string();
    assert_eq!(started["state"], json!("offered"));

    let offered = wait_state(&bob, &sid, "offered", 10).await;
    assert_eq!(offered.as_deref(), Some("offered"), "receiver never saw the offer");
    bob.invoke("session.respond", json!({ "sid": sid, "accept": true, "trust": false }))
        .await
        .expect("respond accepted");
    let done = wait_state(&alice, &sid, "done", 30).await;
    assert_eq!(done.as_deref(), Some("done"), "sender never reached done");

    // 3. Receiver got the exact bytes under the staged name.
    let s = bob.engine.settings();
    let out = PathBuf::from(&s.download_folder).join(&s.save_location).join("photo.bin");
    assert_eq!(std::fs::read(&out).expect("received file exists"), payload);

    // 4. The staged copy is removed once the transfer ends.
    let until = Instant::now() + Duration::from_secs(5);
    while Instant::now() < until && staged_path.exists() {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(!staged_path.exists(), "staged file must be cleaned up after the transfer");
    assert!(!staged_path.parent().unwrap().exists(), "staging folder must be cleaned up too");

    // 5. The returned path works as a normal item source as well.
    let staged2 = alice
        .invoke("files.stage", json!({ "name": "second.bin", "content": b64(&payload[..4096]) }))
        .await
        .expect("second stage");
    let started = alice
        .invoke("transfer.start", json!({ "peer": bob_id, "items": [{ "path": staged2["path"] }] }))
        .await
        .expect("transfer.start accepted the staged path");
    let sid2 = started["sid"].as_str().expect("sid2").to_string();
    assert_eq!(wait_state(&bob, &sid2, "offered", 10).await.as_deref(), Some("offered"));
    bob.invoke("session.respond", json!({ "sid": sid2, "accept": true, "trust": false }))
        .await
        .expect("respond accepted");
    assert_eq!(wait_state(&alice, &sid2, "done", 30).await.as_deref(), Some("done"));
    let out2 = PathBuf::from(&s.download_folder).join(&s.save_location).join("second.bin");
    assert_eq!(std::fs::read(&out2).expect("second file received"), &payload[..4096]);

    alice.stop().await;
    bob.stop().await;
}

#[tokio::test]
async fn unknown_staged_id_is_not_sent() {
    let dir = TestDir::new("unknown");
    let alice = node("a", "Alice Laptop", &dir).await;
    let bob = node("b", "Bob Desktop", &dir).await;
    introduce(&alice, &bob);
    let bob_id = bob.engine.identity.device_id.clone();

    let e = alice
        .invoke("transfer.start", json!({ "peer": bob_id, "items": [{ "stagedId": "stg_deadbeef" }] }))
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound.as_str());

    // A staged id cannot be a traversal either: it is a map key, never a path.
    let e = alice
        .invoke("transfer.start", json!({ "peer": bob_id, "items": [{ "stagedId": "../../etc/passwd" }] }))
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound.as_str());
    alice.stop().await;
    bob.stop().await;
}

#[tokio::test]
async fn traversal_names_are_rejected_and_never_escape_the_staging_folder() {
    let dir = TestDir::new("traversal");
    let (alice, d) = {
        let b = node("a", "Alice Laptop", &dir).await;
        (b, data_dir(&dir, "a"))
    };
    let payload = b"escape attempt";
    let before: Vec<PathBuf> = ["evil.txt", "staging/evil.txt", "staging/stg_x/evil.txt"]
        .iter()
        .map(|p| d.join(p))
        .collect();

    for bad in ["../../evil.txt", "..\\..\\evil.txt", "a/b.txt", "..", ".", "/etc/passwd"] {
        let e = alice
            .invoke("files.stage", json!({ "name": bad, "content": b64(payload) }))
            .await
            .unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidParams.as_str(), "{bad:?} must be rejected with INVALID_PARAMS");
        for p in &before {
            assert!(!p.exists(), "{bad:?} wrote outside the staging folder: {}", p.display());
        }
    }
    // Nothing at all was staged, so the staging folder holds no entries.
    let root = d.join("staging");
    let entries = std::fs::read_dir(&root).map(|r| r.count()).unwrap_or(0);
    assert_eq!(entries, 0, "rejected names must not leave files behind");
    // And nothing appeared next to the data dir (the `../..` target).
    let outside = dir.0.parent().map(|p| p.join("evil.txt"));
    if let Some(p) = outside {
        assert!(!p.exists(), "traversal reached outside the test dir");
    }
    alice.stop().await;
}

#[tokio::test]
async fn oversize_content_is_rejected_with_payload_too_large() {
    let dir = TestDir::new("oversize");
    let alice = node("a", "Alice Laptop", &dir).await;

    let too_big = vec![0u8; dg_core::staging::MAX_STAGE_BYTES + 1];
    let e = alice
        .invoke("files.stage", json!({ "name": "big.bin", "content": b64(&too_big) }))
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::PayloadTooLarge.as_str(), "message: {}", e.message);
    assert!(e.message.contains("staging limit"), "clear message, got: {}", e.message);

    // A file exactly at the cap is fine (boundary check on the decoded size).
    let at_cap = vec![7u8; dg_core::staging::MAX_STAGE_BYTES];
    let ok = alice
        .invoke("files.stage", json!({ "name": "cap.bin", "content": b64(&at_cap) }))
        .await
        .expect("exactly at the cap is accepted");
    assert_eq!(ok["size"], json!(dg_core::staging::MAX_STAGE_BYTES));
    alice.stop().await;
}

#[tokio::test]
async fn unused_staged_files_are_removed_after_the_ttl() {
    let dir = TestDir::new("ttl");
    let alice = node("a", "Alice Laptop", &dir).await;
    let staged = alice
        .invoke("files.stage", json!({ "name": "old.txt", "content": b64(b"stale") }))
        .await
        .expect("stage");
    let id = staged["id"].as_str().unwrap().to_string();
    let path = PathBuf::from(staged["path"].as_str().unwrap());
    assert!(path.exists());

    // Age the entry past the TTL and let the sweep run (the pump calls this every 10 s).
    {
        let mut st = alice.engine.staging.lock().unwrap();
        let entry = st.files.get_mut(&id).expect("registered");
        entry.created_ms -= dg_core::staging::STAGE_TTL.as_millis() as i64 + 1;
        st.last_sweep_ms = 0;
    }
    dg_core::staging::sweep(&alice.engine);
    assert!(!path.exists(), "an unused staged file must be swept after the TTL");
    assert!(!path.parent().unwrap().exists(), "its folder goes too");
    assert!(dg_core::staging::get(&alice.engine, &id).is_none(), "registry entry removed");
    alice.stop().await;
}

#[tokio::test]
async fn http_rpc_allows_a_large_body_only_for_files_stage() {
    let dir = TestDir::new("httpa");
    let alice = node("a", "Alice Laptop", &dir).await;
    let token = api::load_or_create_token(&data_dir(&dir, "a")).unwrap();
    let (addr, server) = api::serve(alice.engine.clone(), 0, token.clone()).await.unwrap();
    let client = reqwest::Client::new();
    let url = format!("http://{addr}/rpc");

    // 200 KiB of content: the JSON body is well over the 64 KiB limit for other methods.
    let payload = vec![9u8; 200 * 1024];
    let body = json!({ "id": 1, "method": "files.stage", "params": { "name": "big-but-ok.bin", "content": b64(&payload) } });
    let body_len = serde_json::to_vec(&body).unwrap().len();
    assert!(body_len > api::MAX_REQUEST_BYTES, "test body must exceed the normal cap ({body_len})");
    let r = client.post(&url).bearer_auth(&token).json(&body).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["result"]["size"], json!(payload.len()));

    // A different method with the same large body keeps the 64 KiB cap: 413.
    let mut big = json!({ "id": 2, "method": "settings.set", "params": { "theme": "light" } });
    big["params"]["padding"] = json!("x".repeat(200 * 1024));
    let r = client.post(&url).bearer_auth(&token).json(&big).send().await.unwrap();
    assert_eq!(r.status(), 413, "non-stage methods must keep the 64 KiB body cap");

    server.abort();
    alice.stop().await;
}
