//! Web share end-to-end (dropgrab-app-design.md section 4.8): a real HTTPS share server with a
//! pinned certificate. Covers the 6-digit PIN gate, the wrong-PIN lockout, streamed file bytes,
//! the single-session lock, and that an old share's expiry timer cannot stop a newer share.

use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use dg_core::{engine::Config, Backend};
use serde_json::{json, Value};

struct TestDir(PathBuf);
impl TestDir {
    fn new(tag: &str) -> TestDir {
        let p = std::env::temp_dir().join(format!("dg-ws-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).ok();
        TestDir(p)
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn backend(dir: &TestDir, ttl: Duration) -> Backend {
    let mut cfg = Config::desktop(dir.0.join("data"), dir.0.join("downloads"));
    cfg.peer_port = 0;
    cfg.webshare_port = 0;
    cfg.discovery = false;
    cfg.advertise_ip = Some(IpAddr::V4(Ipv4Addr::LOCALHOST));
    cfg.device_name = "WS Test".into();
    cfg.webshare_ttl = ttl;
    Backend::start(cfg).await.expect("backend starts")
}

/// Starts a share of one file; returns (url, pin, file path, bytes written).
async fn start_share(b: &Backend, dir: &TestDir, name: &str) -> (String, String, PathBuf, Vec<u8>) {
    let path = dir.0.join(name);
    // 3 MiB with a non-trivial pattern so truncation or reordering would be caught.
    let bytes: Vec<u8> = (0..3 * 1024 * 1024u32).map(|i| (i.wrapping_mul(31) % 251) as u8).collect();
    std::fs::write(&path, &bytes).unwrap();
    let v: Value = b
        .invoke(
            "webshare.start",
            json!({ "files": [{ "path": path.display().to_string(), "name": name }] }),
        )
        .await
        .expect("webshare.start");
    let url = v["url"].as_str().unwrap().to_string();
    let pin = v["pin"].as_str().unwrap().to_string();
    assert_eq!(pin.len(), 6, "pin is 6 digits");
    assert_eq!(v["total"].as_u64(), Some(bytes.len() as u64));
    (url, pin, path, bytes)
}

fn client(b: &Backend) -> reqwest::Client {
    dg_core::identity::pinned_client(&b.engine.identity.fingerprint, Duration::from_secs(10))
        .expect("pinned client")
}

fn wrong_pin(pin: &str) -> String {
    // Any 6-digit value that is not the real PIN.
    let n: u32 = pin.parse().unwrap();
    format!("{:06}", (n + 1) % 1_000_000)
}

#[tokio::test]
async fn pin_gate_lockout_and_streamed_download() {
    let dir = TestDir::new("pin");
    let b = backend(&dir, Duration::from_secs(60)).await;
    let (url, pin, _path, bytes) = start_share(&b, &dir, "photo.bin").await;
    let http = client(&b);
    let base = url.trim_end_matches('/').to_string(); // https://127.0.0.1:port/s/<token>
    let token = base.rsplit('/').next().unwrap().to_string();
    let unlock = |p: &str| format!("{base}/unlock/{p}");

    // The page is served over HTTPS with the pinned certificate.
    let page = http.get(&base).send().await.expect("page reachable");
    assert!(page.status().is_success(), "page status {}", page.status());

    // Files are locked until the PIN is accepted.
    let locked = http.get(format!("{base}/files")).send().await.unwrap();
    assert_eq!(locked.status().as_u16(), 404, "file list hidden before unlock");

    // Five wrong PINs are refused with 403, then the share locks out even the right PIN.
    for _ in 0..5 {
        let r = http.get(unlock(&wrong_pin(&pin))).send().await.unwrap();
        assert_eq!(r.status().as_u16(), 403);
    }
    let blocked = http.get(unlock(&pin)).send().await.unwrap();
    assert_eq!(blocked.status().as_u16(), 429, "correct PIN refused after lockout");
    b.stop().await;

    // Second share: correct PIN unlocks, file streams back byte-for-byte.
    let b = backend(&dir, Duration::from_secs(60)).await;
    let (url, pin, _path, bytes2) = start_share(&b, &dir, "clip.bin").await;
    let base = url.trim_end_matches('/').to_string();
    let http = client(&b);
    let ok = http.get(format!("{base}/unlock/{pin}")).send().await.unwrap();
    assert!(ok.status().is_success(), "correct PIN unlocks: {}", ok.status());
    let listing: Value = ok.json().await.unwrap();
    assert_eq!(listing["files"][0]["name"], "clip.bin");
    assert_eq!(listing["files"][0]["size"], bytes2.len() as u64);

    let got = http.get(format!("{base}/file/0")).send().await.unwrap();
    assert!(got.status().is_success());
    let cd = got.headers()["content-disposition"].to_str().unwrap().to_string();
    assert!(cd.contains("clip.bin"), "disposition {cd}");
    let body = got.bytes().await.unwrap();
    assert_eq!(body.len(), bytes2.len());
    assert!(body.as_ref() == bytes2.as_slice(), "downloaded bytes match");

    // Single-session lock: a second unlock while the first session is open is refused.
    let second = http.get(format!("{base}/unlock/{pin}")).send().await.unwrap();
    assert_eq!(second.status().as_u16(), 409);

    // Unknown token is never served.
    let bogus = http.get(format!("{}/s/nope", base.rsplit_once("/s/").unwrap().0)).send().await.unwrap();
    assert_eq!(bogus.status().as_u16(), 404);
    let _ = (url, bytes, token);
    b.stop().await;
}

#[tokio::test]
async fn stale_expiry_timer_does_not_stop_a_newer_share() {
    let dir = TestDir::new("stale");
    let ttl = Duration::from_secs(4);
    let b = backend(&dir, ttl).await;
    let http = reqwest::Client::builder().danger_accept_invalid_certs(true).build().unwrap();

    // Share A starts at t=0 and is stopped at once; its timer is still pending (fires at ~4 s).
    let (url_a, _pin_a, _, _) = start_share(&b, &dir, "a.bin").await;
    let stop = b.invoke("webshare.stop", json!({})).await.unwrap();
    assert_eq!(stop["stopped"], true);

    // Share B starts at ~2.5 s and runs until ~6.5 s.
    tokio::time::sleep(Duration::from_millis(2500)).await;
    let t0 = Instant::now();
    let (url_b, _pin_b, _, _) = start_share(&b, &dir, "b.bin").await;
    assert_ne!(url_a, url_b);

    // Wait past A's old timer (~4 s after its start) but before B expires (~4 s after its start).
    // A's timer fires at ~1.5 s after t0; B expires at ~4 s after t0. Check at 2.5 s.
    let wake = t0 + Duration::from_millis(2500);
    tokio::time::sleep(wake.saturating_duration_since(Instant::now())).await;
    let alive = http.get(&url_b).send().await.expect("share B reachable");
    assert!(alive.status().is_success(), "newer share survives the old timer: {}", alive.status());

    // After B's own TTL it is gone.
    tokio::time::sleep(Duration::from_millis(3000)).await;
    // Stopped share: the listener is shut down, so the request fails to connect.
    let gone = http.get(&url_b).send().await;
    assert!(gone.is_err(), "share B stopped after its TTL, got {:?}", gone.map(|r| r.status()));
    b.stop().await;
}
