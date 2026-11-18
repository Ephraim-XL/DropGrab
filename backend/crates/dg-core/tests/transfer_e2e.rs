//! Transfer end-to-end (dropgrab-app-design.md section 4): two real engines in one process talk
//! over the peer HTTPS listener (TLS, fingerprint pinning, PUT ranges, completion, SHA-256 check).

use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use dg_core::{engine::Config, Backend};
use serde_json::{json, Value};

struct TestDir(PathBuf);
impl TestDir {
    fn new(tag: &str) -> TestDir {
        let p = std::env::temp_dir().join(format!("dg-e2e-{tag}-{}", uuid::Uuid::new_v4()));
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
    cfg.chunk_size = 64 * 1024; // several PUTs for a small file
    Backend::start(cfg).await.expect("engine starts")
}

/// Makes `b` appear in `a`'s peer table, the way discovery would after an announce.
fn introduce(a: &Backend, b: &Backend) {
    let info = b.engine.info();
    let peer = a.engine.peer_from_info(&info, IpAddr::V4(Ipv4Addr::LOCALHOST), b.engine.peer_port);
    a.engine.note_peer(peer);
}

/// Polls `session.history` (which keeps terminal sessions) until `sid` reaches `want`.
/// Returns the last state seen.
async fn wait_state(b: &Backend, sid: &str, want: &str, secs: u64) -> Option<String> {
    let until = Instant::now() + Duration::from_secs(secs);
    let mut last = None;
    while Instant::now() < until {
        let v = b.invoke("session.history", json!({})).await.ok()?;
        last = sessions_state(&v, sid);
        if last.as_deref() == Some(want) {
            return last;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    last
}

fn sessions_state(v: &Value, sid: &str) -> Option<String> {
    let arr = v.get("sessions")?.as_array()?;
    arr.iter()
        .find(|s| s.get("sid").and_then(Value::as_str) == Some(sid))
        .and_then(|s| s.get("state").and_then(Value::as_str).map(str::to_string))
}

#[tokio::test]
async fn send_file_accept_and_verify_bytes() {
    let dir = TestDir::new("xfer");
    let alice = node("a", "Alice Laptop", &dir).await;
    let bob = node("b", "Bob Desktop", &dir).await;
    introduce(&alice, &bob);

    // 300 KiB of deterministic content: several 64 KiB chunks.
    let src = dir.0.join("photo.bin");
    let payload: Vec<u8> = (0..300 * 1024u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
    std::fs::write(&src, &payload).unwrap();

    let bob_id = bob.engine.identity.device_id.clone();
    let sent = alice
        .invoke("session.send", json!({ "peer": bob_id, "items": [{ "path": src.display().to_string() }] }))
        .await
        .expect("session.send accepted");
    let sid = sent["sid"].as_str().expect("sid").to_string();

    // Bob sees the offer and must answer it; we accept without trusting.
    let offered = wait_state(&bob, &sid, "offered", 10)
        .await
        .expect("receiver never listed the offered session");
    assert_eq!(offered, "offered", "receiver never saw the offer");

    bob.invoke("session.respond", json!({ "sid": sid, "accept": true, "trust": false }))
        .await
        .expect("respond accepted");

    let done = wait_state(&alice, &sid, "done", 30).await;
    assert_eq!(done.as_deref(), Some("done"), "sender never reached done");

    // Bytes on disk match the source, byte for byte (section 13: files land in
    // downloadFolder/saveLocation, here `<downloads>/Received`).
    let s = bob.engine.settings();
    let out = PathBuf::from(&s.download_folder).join(&s.save_location).join("photo.bin");
    let got = std::fs::read(&out).expect("received file exists");
    assert_eq!(got.len(), payload.len());
    assert_eq!(got, payload);

    // The receiver also indexed the finished file (section 7: library.changed on completion).
    let lib = bob.invoke("library.query", json!({})).await.expect("library.query");
    let names: Vec<String> = lib["items"]
        .as_array()
        .map(|a| a.iter().filter_map(|i| i["name"].as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    assert!(names.iter().any(|n| n == "photo.bin"), "received file not in library: {names:?}");

    alice.stop().await;
    bob.stop().await;
}

#[tokio::test]
async fn pinned_fingerprint_mismatch_is_refused() {
    // A client pinned to the wrong certificate must fail the TLS handshake (section 4.4).
    let dir = TestDir::new("pin");
    let bob = node("b", "Bob", &dir).await;
    let wrong = "00".repeat(32);
    let client = dg_core::identity::pinned_client(&wrong, Duration::from_secs(5)).unwrap();
    let url = format!("https://127.0.0.1:{}/api/v1/info", bob.engine.peer_port);
    let res = client.get(&url).send().await;
    assert!(res.is_err(), "pinned request to a different certificate must fail");

    // The same client pinned to the real fingerprint succeeds.
    let good = dg_core::identity::pinned_client(&bob.engine.identity.fingerprint, Duration::from_secs(5)).unwrap();
    let ok = good.get(&url).send().await.expect("pinned request succeeds");
    assert_eq!(ok.status(), 200);
    bob.stop().await;
}
