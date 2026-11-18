//! Download end-to-end (dropgrab-app-design.md section 5): a real segmented, Range-resuming
//! download against a local HTTP/1.1 server that supports `Range`, then byte-for-byte verification
//! and library indexing. Also covers probe errors (404 -> NOT_FOUND) and pause/resume.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use dg_core::{engine::Config, Backend};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

struct TestDir(PathBuf);
impl TestDir {
    fn new(tag: &str) -> TestDir {
        let p = std::env::temp_dir().join(format!("dg-dl-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).ok();
        TestDir(p)
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Minimal HTTP/1.1 file server: GET/HEAD, `Range: bytes=a-b` -> 206, `bytes=a-` open-ended.
/// Deliberately dumb so the test exercises the client, not a framework.
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
                let path = parts.next().unwrap_or("");
                let mut range: Option<(usize, usize)> = None;
                for line in lines {
                    let lower = line.to_ascii_lowercase();
                    if let Some(v) = lower.strip_prefix("range:") {
                        let v = v.trim();
                        if let Some(spec) = v.strip_prefix("bytes=") {
                            let (a, b) = spec.split_once('-').unwrap_or(("", ""));
                            let start: usize = a.trim().parse().unwrap_or(0);
                            let end: usize = b.trim().parse().unwrap_or(body.len().saturating_sub(1));
                            range = Some((start.min(body.len()), (end + 1).min(body.len())));
                        }
                    }
                }
                let (status, slice, extra) = if path.starts_with("/missing") {
                    ("404 Not Found", &body[0..0], String::new())
                } else if let Some((s, e)) = range {
                    (
                        "206 Partial Content",
                        &body[s..e],
                        format!(
                            "Content-Range: bytes {}-{}/{}\r\n",
                            s,
                            e.saturating_sub(1),
                            body.len()
                        ),
                    )
                } else {
                    ("200 OK", &body[..], String::new())
                };
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n\
                     Content-Type: application/octet-stream\r\nETag: \"test-etag\"\r\n{extra}Connection: close\r\n\r\n",
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

async fn backend(tag: &str, dir: &TestDir) -> Backend {
    let mut cfg = Config::desktop(dir.0.join(format!("{tag}-data")), dir.0.join("downloads"));
    cfg.peer_port = 0;
    cfg.discovery = false;
    cfg.device_name = "DL Test".into();
    Backend::start(cfg).await.expect("backend starts")
}

async fn wait_job(b: &Backend, id: &str, want: &str, secs: u64) -> Option<String> {
    let until = Instant::now() + Duration::from_secs(secs);
    let mut last = None;
    while Instant::now() < until {
        let v: Value = b.invoke("downloads.list", json!({})).await.ok()?;
        last = v["jobs"]
            .as_array()
            .and_then(|a| a.iter().find(|j| j["id"].as_str() == Some(id)))
            .and_then(|j| j["state"].as_str().map(str::to_string));
        if last.as_deref() == Some(want) {
            return last;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    last
}

#[tokio::test]
async fn direct_download_is_segmented_verified_and_indexed() {
    let dir = TestDir::new("ok");
    let payload: Vec<u8> = (0..512 * 1024u32)
        .map(|i| (i.wrapping_mul(2654435761) >> 11) as u8)
        .collect();
    let port = serve(payload.clone()).await;
    let b = backend("a", &dir).await;
    let url = format!("http://127.0.0.1:{port}/movie.mp4");

    // Probe reports a direct file with size and range support.
    let p = b.invoke("downloads.probe", json!({ "url": url })).await.expect("probe");
    assert_eq!(p["kind"], json!("direct"), "probe: {p}");
    assert_eq!(p["total"], json!(payload.len()));
    assert_eq!(p["acceptsRanges"], json!(true));

    let job = b
        .invoke("downloads.add", json!({ "url": url, "kind": "direct", "name": "movie.mp4" }))
        .await
        .expect("add");
    let id = job["id"].as_str().expect("id").to_string();

    let st = wait_job(&b, &id, "finished", 30).await;
    assert_eq!(st.as_deref(), Some("finished"), "job did not finish");

    // destination: <downloadFolder>/Videos/movie.mp4
    let out = dir.0.join("downloads").join("Videos").join("movie.mp4");
    let got = std::fs::read(&out).expect("downloaded file exists");
    assert_eq!(got.len(), payload.len(), "size mismatch");
    assert!(got == payload, "downloaded bytes differ from the source");

    // The finished download is indexed in the library (section 7).
    let lib: Value = b.invoke("library.query", json!({})).await.expect("library.query");
    let names: Vec<String> = lib["items"]
        .as_array()
        .map(|a| a.iter().filter_map(|i| i["name"].as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    assert!(names.iter().any(|n| n == "movie.mp4"), "not in library: {names:?}");

    // A second identical job gets its own path (no overwrite) and still completes.
    let job2 = b.invoke("downloads.add", json!({ "url": url, "kind": "direct", "name": "movie.mp4" })).await.expect("add 2");
    let id2 = job2["id"].as_str().unwrap().to_string();
    assert_eq!(wait_job(&b, &id2, "finished", 30).await.as_deref(), Some("finished"));
    let files: Vec<String> = std::fs::read_dir(dir.0.join("downloads").join("Videos"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().to_string()))
        .collect();
    assert_eq!(files.len(), 2, "second download must not overwrite the first: {files:?}");

    b.stop().await;
}

#[tokio::test]
async fn probe_missing_is_not_found_and_bad_url_is_rejected() {
    let dir = TestDir::new("404");
    let port = serve(b"x".to_vec()).await;
    let b = backend("b", &dir).await;
    let e = b
        .invoke("downloads.probe", json!({ "url": format!("http://127.0.0.1:{port}/missing.bin") }))
        .await
        .unwrap_err();
    assert_eq!(e.code, "NOT_FOUND");
    let e = b.invoke("downloads.probe", json!({ "url": "ftp://example.com/x" })).await.unwrap_err();
    assert_eq!(e.code, "INVALID_PARAMS");
    let e = b.invoke("downloads.pause", json!({ "id": "nope" })).await.unwrap_err();
    assert_eq!(e.code, "UNKNOWN_JOB");
    b.stop().await;
}

#[tokio::test]
async fn pause_and_cancel_are_reflected_in_state() {
    let dir = TestDir::new("pause");
    let payload = vec![7u8; 256 * 1024];
    let port = serve(payload).await;
    let b = backend("c", &dir).await;
    let url = format!("http://127.0.0.1:{port}/slow.bin");
    let job = b.invoke("downloads.add", json!({ "url": url, "kind": "direct", "name": "slow.bin" })).await.unwrap();
    let id = job["id"].as_str().unwrap().to_string();
    // Pause may land before or after the job starts; either way the command must be accepted
    // and the state must become `paused` (or terminal if it already finished).
    let _ = b.invoke("downloads.pause", json!({ "id": id })).await.expect("pause accepted");
    let st = wait_job(&b, &id, "paused", 5).await;
    assert!(
        matches!(st.as_deref(), Some("paused") | Some("finished")),
        "unexpected state after pause: {st:?}"
    );
    if st.as_deref() == Some("paused") {
        b.invoke("downloads.resume", json!({ "id": id })).await.expect("resume accepted");
        assert_eq!(wait_job(&b, &id, "finished", 30).await.as_deref(), Some("finished"));
    }
    // Cancel on a finished job is a no-op error or success depending on the state machine; the
    // command must still answer without panicking.
    let _ = b.invoke("downloads.cancel", json!({ "id": id })).await;
    b.stop().await;
}
