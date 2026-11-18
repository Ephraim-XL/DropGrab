//! Control-API Origin policy (`--allow-origin`): an explicitly configured browser origin may
//! call the API with a valid token; everything else keeps today's behaviour.

use std::path::PathBuf;

use dg_core::{api, engine::Config, Backend};
use serde_json::{json, Value};

struct TestDir(PathBuf);
impl TestDir {
    fn new(tag: &str) -> TestDir {
        let p = std::env::temp_dir().join(format!("dg-origin-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn start(tag: &str) -> (Backend, TestDir, String, String) {
    let dir = TestDir::new(tag);
    let mut cfg = Config::desktop(dir.0.join("data"), dir.0.join("downloads"));
    cfg.peer_port = 0;
    cfg.discovery = false;
    cfg.device_name = "Origin Test".into();
    let backend = Backend::start(cfg).await.expect("backend starts");
    let token = api::load_or_create_token(&dir.0.join("data")).unwrap();
    let (addr, server) = api::serve_with_origins(
        backend.engine.clone(),
        0,
        token.clone(),
        vec!["http://localhost:8080".into()],
    )
    .await
    .unwrap();
    // The serving task is detached: it ends with the test's runtime, and `backend.stop()` only
    // stops the engine's own services.
    std::mem::forget(server);
    (backend, dir, token, format!("http://{addr}/rpc"))
}

#[tokio::test]
async fn allowed_origin_is_accepted_and_gets_cors_headers() {
    let (backend, _dir, token, url) = start("allow").await;
    let client = reqwest::Client::new();
    let r = client
        .post(&url)
        .header("Origin", "http://localhost:8080")
        .bearer_auth(&token)
        .json(&json!({ "id": 1, "method": "backend.info" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "an allowed origin with a good token must be accepted");
    assert_eq!(
        r.headers().get("access-control-allow-origin").map(|v| v.to_str().unwrap()),
        Some("http://localhost:8080")
    );
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["result"]["apiVersion"], json!(dg_types::API_VERSION));

    // Exactly that one origin is allowed; a sibling origin on another port is not.
    let r = client
        .post(&url)
        .header("Origin", "http://localhost:8081")
        .bearer_auth(&token)
        .json(&json!({ "id": 2, "method": "backend.info" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);
    assert!(r.headers().get("access-control-allow-origin").is_none(), "403 must not carry CORS headers");
    backend.stop().await;
}

#[tokio::test]
async fn unlisted_origin_is_forbidden() {
    let (backend, _dir, token, url) = start("deny").await;
    let client = reqwest::Client::new();
    for origin in ["http://evil.example", "null", "https://localhost:8080"] {
        let r = client
            .post(&url)
            .header("Origin", origin)
            .bearer_auth(&token)
            .json(&json!({ "id": 3, "method": "backend.info" }))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 403, "{origin} must be rejected even with a valid token");
    }
    backend.stop().await;
}

#[tokio::test]
async fn origin_less_requests_still_need_only_the_token() {
    let (backend, _dir, token, url) = start("none").await;
    let client = reqwest::Client::new();
    let r = client
        .post(&url)
        .bearer_auth(&token)
        .json(&json!({ "id": 4, "method": "devices.list" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "no Origin header (native client) keeps working");
    assert!(r.headers().get("access-control-allow-origin").is_none());
    backend.stop().await;
}

#[tokio::test]
async fn bad_token_is_unauthorized_even_for_an_allowed_origin() {
    let (backend, _dir, _token, url) = start("token").await;
    let client = reqwest::Client::new();
    let r = client
        .post(&url)
        .header("Origin", "http://localhost:8080")
        .bearer_auth("not-the-token")
        .json(&json!({ "id": 5, "method": "backend.info" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
    let r = client
        .post(&url)
        .header("Origin", "http://localhost:8080")
        .json(&json!({ "id": 6, "method": "backend.info" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401, "missing token is 401 too");
    backend.stop().await;
}

#[tokio::test]
async fn default_policy_rejects_every_browser_origin() {
    let dir = TestDir::new("default");
    let mut cfg = Config::desktop(dir.0.join("data"), dir.0.join("downloads"));
    cfg.peer_port = 0;
    cfg.discovery = false;
    let backend = Backend::start(cfg).await.unwrap();
    let token = api::load_or_create_token(&dir.0.join("data")).unwrap();
    let (addr, server) = api::serve(backend.engine.clone(), 0, token.clone()).await.unwrap();
    let url = format!("http://{addr}/rpc");
    let client = reqwest::Client::new();

    let r = client
        .post(&url)
        .header("Origin", "http://localhost:8080")
        .bearer_auth(&token)
        .json(&json!({ "id": 7, "method": "backend.info" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403, "deny-all stays the default");
    // Origin-less callers are unaffected.
    let r = client
        .post(&url)
        .bearer_auth(&token)
        .json(&json!({ "id": 8, "method": "backend.info" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    server.abort();
    backend.stop().await;
}

#[tokio::test]
async fn cors_preflight_follows_the_same_allowlist() {
    let (backend, _dir, _token, url) = start("preflight").await;
    let client = reqwest::Client::new();

    let r = client
        .request(reqwest::Method::OPTIONS, &url)
        .header("Origin", "http://localhost:8080")
        .header("Access-Control-Request-Method", "POST")
        .header("Access-Control-Request-Headers", "authorization, content-type")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 204, "preflight from an allowed origin must succeed");
    assert_eq!(
        r.headers().get("access-control-allow-origin").map(|v| v.to_str().unwrap()),
        Some("http://localhost:8080")
    );
    let allowed = r.headers().get("access-control-allow-headers").map(|v| v.to_str().unwrap()).unwrap_or("");
    assert!(allowed.contains("authorization"), "preflight must allow the Authorization header: {allowed}");
    assert!(allowed.contains("content-type"), "preflight must allow Content-Type: {allowed}");

    let r = client
        .request(reqwest::Method::OPTIONS, &url)
        .header("Origin", "http://evil.example")
        .header("Access-Control-Request-Method", "POST")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);
    backend.stop().await;
}

#[test]
fn origins_are_normalized() {
    assert_eq!(api::normalize_origin(" http://localhost:8080/ "), "http://localhost:8080");
    assert_eq!(api::normalize_origin("http://localhost:8080"), "http://localhost:8080");
}
