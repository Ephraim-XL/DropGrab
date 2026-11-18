//! Regression for the download scheduler: a probe that panics must still release the job's
//! concurrency slot, and `downloads.cancel` must be able to free a job stuck in `probing`.
//!
//! With `downloadsAtOnce = 1` a leaked slot blocks every later job, so the second job reaching
//! `finished` is the assertion that the slot came back.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use dg_core::{engine::Config, Backend};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

struct TestDir(PathBuf);
impl TestDir {
    fn new(tag: &str) -> TestDir {
        let p = std::env::temp_dir().join(format!("dg-slot-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).ok();
        TestDir(p)
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Minimal HTTP/1.1 file server with `Range` support (enough for the queue to finish a job).
async fn serve(payload: Vec<u8>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (mut sock, _) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => break,
            };
            let body = payload.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let n = match sock.read(&mut buf).await {
                    Ok(n) if n > 0 => n,
                    _ => return,
                };
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let mut lines = req.split("\r\n");
                let first = lines.next().unwrap_or("");
                let mut parts = first.split(' ');
                let method = parts.next().unwrap_or("");
                let mut range: Option<(usize, usize)> = None;
                for line in lines {
                    let lower = line.to_ascii_lowercase();
                    if let Some(v) = lower.strip_prefix("range:") {
                        if let Some(spec) = v.trim().strip_prefix("bytes=") {
                            let (a, b) = spec.split_once('-').unwrap_or(("", ""));
                            let start: usize = a.trim().parse().unwrap_or(0);
                            let end: usize = b.trim().parse().unwrap_or(body.len().saturating_sub(1));
                            range = Some((start.min(body.len()), (end + 1).min(body.len())));
                        }
                    }
                }
                let (status, slice, extra) = match range {
                    Some((s, e)) => (
                        "206 Partial Content",
                        &body[s..e],
                        format!("Content-Range: bytes {}-{}/{}\r\n", s, e.saturating_sub(1), body.len()),
                    ),
                    None => ("200 OK", &body[..], String::new()),
                };
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n\
                     Content-Type: application/octet-stream\r\nETag: \"slot-etag\"\r\n{extra}Connection: close\r\n\r\n",
                    slice.len()
                );
                let _ = sock.write_all(head.as_bytes()).await;
                if method != "HEAD" {
                    let _ = sock.write_all(slice).await;
                }
                let _ = sock.shutdown().await;
            });
        }
    });
    port
}

/// Accepts connections and never answers, so a job stays in `probing`.
async fn hang() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((sock, _)) => {
                    tokio::spawn(async move {
                        // Hold the socket open with no response.
                        tokio::time::sleep(Duration::from_secs(60)).await;
                        drop(sock);
                    });
                }
                Err(_) => break,
            }
        }
    });
    port
}

async fn backend(tag: &str, dir: &TestDir) -> Backend {
    let mut cfg = Config::desktop(dir.0.join(format!("{tag}-data")), dir.0.join("downloads"));
    cfg.peer_port = 0;
    cfg.discovery = false;
    cfg.device_name = "Slot Test".into();
    let b = Backend::start(cfg).await.expect("backend starts");
    b.invoke("settings.set", json!({ "downloadsAtOnce": 1 }))
        .await
        .expect("one download at a time");
    b
}

async fn job_state(b: &Backend, id: &str) -> Option<String> {
    let v: Value = b.invoke("downloads.list", json!({})).await.ok()?;
    v["jobs"]
        .as_array()
        .and_then(|a| a.iter().find(|j| j["id"].as_str() == Some(id)))
        .and_then(|j| j["state"].as_str().map(str::to_string))
}

async fn wait_state(b: &Backend, id: &str, want: &str, secs: u64) -> Option<String> {
    let until = Instant::now() + Duration::from_secs(secs);
    let mut last = None;
    while Instant::now() < until {
        last = job_state(b, id).await;
        if last.as_deref() == Some(want) {
            return last;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    last
}

#[tokio::test]
async fn panicking_probe_releases_the_slot_for_later_downloads() {
    let dir = TestDir::new("panic");
    let payload: Vec<u8> = (0..128 * 1024u32).map(|i| (i * 31 % 251) as u8).collect();
    let port = serve(payload).await;
    let b = backend("p", &dir).await;

    // The hook makes the probe of any URL containing this tag panic (debug builds only).
    let tag = format!("panic-probe-{}", uuid::Uuid::new_v4());
    std::env::set_var("DROPGRAB_TEST_PANIC_PROBE", &tag);

    let a = b
        .invoke("downloads.add", json!({ "url": format!("http://127.0.0.1:{port}/{tag}.bin"), "kind": "direct" }))
        .await
        .expect("add panicking job");
    let a_id = a["id"].as_str().unwrap().to_string();

    let st = wait_state(&b, &a_id, "failed", 10).await;
    assert_eq!(st.as_deref(), Some("failed"), "a panicked probe must fail the job, got {st:?}");
    let list: Value = b.invoke("downloads.list", json!({})).await.unwrap();
    let err = list["jobs"]
        .as_array()
        .and_then(|a| a.iter().find(|j| j["id"].as_str() == Some(a_id.as_str())))
        .and_then(|j| j["error"].as_str().map(str::to_string));
    assert_eq!(err.as_deref(), Some("INTERNAL"), "panicked job should report INTERNAL");

    // With the slot released, a normal job runs even though only one download is allowed at once.
    let url = format!("http://127.0.0.1:{port}/after-panic.bin");
    let j = b
        .invoke("downloads.add", json!({ "url": url, "kind": "direct", "name": "after-panic.bin" }))
        .await
        .expect("add second job");
    let id = j["id"].as_str().unwrap().to_string();
    let st = wait_state(&b, &id, "finished", 30).await;
    assert_eq!(st.as_deref(), Some("finished"), "later download was blocked by the leaked slot: {st:?}");

    std::env::remove_var("DROPGRAB_TEST_PANIC_PROBE");
    b.stop().await;
}

#[tokio::test]
async fn cancel_frees_a_probing_job_and_its_slot() {
    let dir = TestDir::new("cancel-probe");
    let payload = vec![9u8; 64 * 1024];
    let port = serve(payload).await;
    let hang_port = hang().await;
    let b = backend("c", &dir).await;

    let a = b
        .invoke("downloads.add", json!({ "url": format!("http://127.0.0.1:{hang_port}/stuck.bin"), "kind": "direct" }))
        .await
        .expect("add stuck job");
    let a_id = a["id"].as_str().unwrap().to_string();
    let st = wait_state(&b, &a_id, "probing", 5).await;
    assert_eq!(st.as_deref(), Some("probing"), "job should be probing against a silent server: {st:?}");

    b.invoke("downloads.cancel", json!({ "id": a_id })).await.expect("cancel probing job");
    let gone = job_state(&b, &a_id).await;
    assert_eq!(gone, None, "a cancelled probing job must leave the queue, got {gone:?}");

    let j = b
        .invoke("downloads.add", json!({ "url": format!("http://127.0.0.1:{port}/after-cancel.bin"), "kind": "direct", "name": "after-cancel.bin" }))
        .await
        .expect("add after cancel");
    let id = j["id"].as_str().unwrap().to_string();
    let st = wait_state(&b, &id, "finished", 30).await;
    assert_eq!(st.as_deref(), Some("finished"), "slot was not freed by cancel: {st:?}");

    b.stop().await;
}
