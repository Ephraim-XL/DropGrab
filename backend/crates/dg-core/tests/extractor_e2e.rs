//! Extractor regression: an oversized HTML page whose byte `MAX_HTML` falls inside a multi-byte
//! character used to panic (`&body[..MAX_HTML]`). The page must resolve normally instead.
//!
//! This drives `extractor::resolve` end to end over a real HTTP server, so it also covers the
//! capped body read (`read_html_body`).

use std::path::PathBuf;

use dg_core::{engine::Config, extractor, Backend};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const MAX_HTML: usize = 2 * 1024 * 1024;

struct TestDir(PathBuf);
impl TestDir {
    fn new(tag: &str) -> TestDir {
        let p = std::env::temp_dir().join(format!("dg-extract-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).ok();
        TestDir(p)
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Serves one fixed HTML body on every connection.
async fn serve_page(body: Vec<u8>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (mut sock, _) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => break,
            };
            let body = body.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let _ = sock.read(&mut buf).await;
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Connection: close\r\n\r\n",
                    body.len()
                );
                if sock.write_all(head.as_bytes()).await.is_err() {
                    return;
                }
                let _ = sock.write_all(&body).await;
                let _ = sock.shutdown().await;
            });
        }
    });
    port
}

#[tokio::test]
async fn oversized_multibyte_page_resolves_without_panicking() {
    let dir = TestDir::new("big");
    let mut cfg = Config::desktop(dir.0.join("data"), dir.0.join("downloads"));
    cfg.peer_port = 0;
    cfg.discovery = false;
    let b = Backend::start(cfg).await.expect("backend starts");

    let mut body = String::from(
        "<html><head><meta property=\"og:title\" content=\"Big page\"><title>Big page</title>\
         </head><body><video src=\"/media/m.mp4\"></video>",
    );
    // Align the prefix so the 2 MiB cut lands inside a 3-byte character.
    while body.len() % 3 != 0 {
        body.push('x');
    }
    body.push_str(&"€".repeat(MAX_HTML)); // ~6 MiB of multi-byte text
    body.push_str("</body></html>");
    assert!(body.len() > MAX_HTML);
    assert!(!body.is_char_boundary(MAX_HTML), "test setup must cut inside a character");

    let port = serve_page(body.into_bytes()).await;
    let url = format!("http://127.0.0.1:{port}/big.html");
    let r = extractor::resolve(&b.engine, &url).await.expect("oversized page must resolve, not panic");
    assert_eq!(r.title, "Big page");
    assert_eq!(r.formats.len(), 1, "formats: {:?}", r.formats);
    assert_eq!(r.formats[0]["url"], format!("http://127.0.0.1:{port}/media/m.mp4"));

    b.stop().await;
}
