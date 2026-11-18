//! Mutual TLS end-to-end (dropgrab-app-design.md section 4.4). The peer's claimed sender
//! fingerprint must equal the client certificate it presents on the TLS handshake. Auto-accept
//! and trust decisions depend on that fingerprint, so a mismatched or missing client certificate
//! is refused on every peer route that trusts a claim.
//!
//! Each refusal test is paired with a positive control (the same request with the right
//! certificate succeeds), so a refusal is not passing just because everything is broken.

use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::time::Duration;

use dg_core::{engine::Config, identity, Backend};
use dg_types::{OfferRequest, Sender, TextRequest};
use serde_json::{json, Value};

struct TestDir(PathBuf);
impl TestDir {
    fn new(tag: &str) -> TestDir {
        let p = std::env::temp_dir().join(format!("dg-mtls-{tag}-{}", uuid::Uuid::new_v4()));
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
    cfg.offer_timeout = Duration::from_millis(300);
    Backend::start(cfg).await.expect("engine starts")
}

fn base(b: &Backend) -> String {
    format!("https://127.0.0.1:{}", b.engine.peer_port)
}

fn sender_of(b: &Backend) -> Sender {
    b.engine.self_sender()
}

/// A client pinned to `b`'s real certificate and presenting `who`'s certificate (mutual TLS).
fn client_as(who: &Backend, target: &Backend) -> reqwest::Client {
    identity::mtls_client(&who.engine.identity, &target.engine.identity.fingerprint, Duration::from_secs(5))
        .expect("mtls client")
}

/// A pinned client that presents no client certificate.
fn client_anon(target: &Backend) -> reqwest::Client {
    identity::pinned_client(&target.engine.identity.fingerprint, Duration::from_secs(5)).expect("pinned client")
}

fn one_file_offer(sender: Sender) -> OfferRequest {
    OfferRequest {
        sid: None,
        sender,
        files: vec![dg_types::FileMeta {
            fid: "f1".into(),
            name: "a.txt".into(),
            size: 5,
            mime: "text/plain".into(),
            sha256: None,
            modified: None,
        }],
        total_size: 5,
    }
}

fn text_req(sender: Sender, text: &str) -> TextRequest {
    TextRequest { sender, text: text.into() }
}

/// Event kinds emitted by `b` so far, from the replay ring.
fn event_kinds(b: &Backend) -> Vec<String> {
    match b.engine.bus.replay(0) {
        dg_core::events::Replay::Events(evs) => evs.into_iter().map(|e| e.kind).collect(),
        _ => Vec::new(),
    }
}

#[tokio::test]
async fn offer_with_matching_client_cert_is_accepted_for_auto_accept() {
    // Positive control: alice presents her real certificate and claims her real fingerprint.
    // Bob has allowed auto-accept for alice, so the offer is accepted without a prompt.
    let dir = TestDir::new("ok");
    let alice = node("a", "Alice", &dir).await;
    let bob = node("b", "Bob", &dir).await;
    bob.engine
        .upsert_device(&alice.engine.identity.device_id, "Alice", "laptop", "", &alice.engine.identity.fingerprint)
        .unwrap();
    bob.engine.db.device_set_trust(&alice.engine.identity.device_id, "trusted", Some(true)).unwrap();

    let resp = client_as(&alice, &bob)
        .post(format!("{}/api/v1/session/offer", base(&bob)))
        .json(&one_file_offer(sender_of(&alice)))
        .send()
        .await
        .expect("offer reaches bob");
    assert!(resp.status().is_success(), "matching client cert must be accepted: {}", resp.status());
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["accepted"], json!(true), "auto-accept for a trusted, verified sender");
    alice.stop().await;
    bob.stop().await;
}

#[tokio::test]
async fn offer_with_mismatched_client_cert_is_rejected() {
    // Regression: mallory claims alice's fingerprint but presents her own certificate.
    // Bob must refuse, and must not emit any session event for the forged offer.
    let dir = TestDir::new("mismatch");
    let alice = node("a", "Alice", &dir).await;
    let mallory = node("m", "Mallory", &dir).await;
    let bob = node("b", "Bob", &dir).await;
    bob.engine
        .upsert_device(&alice.engine.identity.device_id, "Alice", "laptop", "", &alice.engine.identity.fingerprint)
        .unwrap();
    bob.engine.db.device_set_trust(&alice.engine.identity.device_id, "trusted", Some(true)).unwrap();

    // Claims alice's identity, presents mallory's certificate.
    let forged = Sender { device_id: alice.engine.identity.device_id.clone(), alias: "Alice".into(), device_type: "laptop".into(), fingerprint: alice.engine.identity.fingerprint.clone() };
    let resp = client_as(&mallory, &bob)
        .post(format!("{}/api/v1/session/offer", base(&bob)))
        .json(&one_file_offer(forged))
        .send()
        .await
        .expect("request reaches bob");
    assert_eq!(resp.status().as_u16(), 403, "a forged sender fingerprint must be refused");
    let body: Value = resp.json().await.unwrap();
    assert!(body["error"].as_str().unwrap_or("").contains("does not match"), "{body}");
    assert!(
        !event_kinds(&bob).iter().any(|k| k == "session.offered"),
        "a refused offer must not reach the user"
    );
    let listed = bob.invoke("session.list", json!({})).await.unwrap();
    assert!(
        listed["sessions"].as_array().map(|a| a.is_empty()).unwrap_or(true),
        "a refused offer must not create a session: {listed}"
    );
    alice.stop().await;
    mallory.stop().await;
    bob.stop().await;
}

#[tokio::test]
async fn offer_without_client_cert_is_rejected() {
    // Regression: the claimed fingerprint is alice's, but the request carries no client
    // certificate (anonymous pinned client). Absent certificate must be refused.
    let dir = TestDir::new("nocert");
    let alice = node("a", "Alice", &dir).await;
    let bob = node("b", "Bob", &dir).await;
    bob.engine
        .upsert_device(&alice.engine.identity.device_id, "Alice", "laptop", "", &alice.engine.identity.fingerprint)
        .unwrap();
    bob.engine.db.device_set_trust(&alice.engine.identity.device_id, "trusted", Some(true)).unwrap();

    let resp = client_anon(&bob)
        .post(format!("{}/api/v1/session/offer", base(&bob)))
        .json(&one_file_offer(sender_of(&alice)))
        .send()
        .await
        .expect("request reaches bob");
    assert_eq!(resp.status().as_u16(), 403, "absent client cert must be refused");
    let body: Value = resp.json().await.unwrap();
    assert!(body["error"].as_str().unwrap_or("").contains("client certificate required"), "{body}");
    assert!(!event_kinds(&bob).iter().any(|k| k == "session.offered"));
    alice.stop().await;
    bob.stop().await;
}

#[tokio::test]
async fn text_with_spoofed_sender_is_rejected_without_event() {
    // Mallory sends text claiming to be alice. Bob must not emit `text.received` for it.
    let dir = TestDir::new("text");
    let alice = node("a", "Alice", &dir).await;
    let mallory = node("m", "Mallory", &dir).await;
    let bob = node("b", "Bob", &dir).await;

    let forged = Sender { device_id: alice.engine.identity.device_id.clone(), alias: "Alice".into(), device_type: "laptop".into(), fingerprint: alice.engine.identity.fingerprint.clone() };
    let resp = client_as(&mallory, &bob)
        .post(format!("{}/api/v1/text", base(&bob)))
        .json(&text_req(forged, "click this"))
        .send()
        .await
        .expect("request reaches bob");
    assert_eq!(resp.status().as_u16(), 403, "spoofed text sender must be refused");
    assert!(!event_kinds(&bob).iter().any(|k| k == "text.received"));

    // Positive control: the real sender's text is delivered.
    let ok = client_as(&alice, &bob)
        .post(format!("{}/api/v1/text", base(&bob)))
        .json(&text_req(sender_of(&alice), "hello"))
        .send()
        .await
        .expect("request reaches bob");
    assert_eq!(ok.status().as_u16(), 204, "matching cert text is accepted");
    assert!(event_kinds(&bob).iter().any(|k| k == "text.received"));
    alice.stop().await;
    mallory.stop().await;
    bob.stop().await;
}

#[tokio::test]
async fn session_route_from_a_different_peer_is_rejected() {
    // Bob has an offer open from alice. Mallory, who has no part in that session, tries to
    // cancel it using the session id. The route must refuse her certificate.
    let dir = TestDir::new("sess");
    let alice = node("a", "Alice", &dir).await;
    let mallory = node("m", "Mallory", &dir).await;
    let bob = node("b", "Bob", &dir).await;

    // Alice's offer waits for a decision (not auto-accepted); keep it open in the background.
    let offer = one_file_offer(sender_of(&alice));
    let alice_client = client_as(&alice, &bob);
    let url = format!("{}/api/v1/session/offer", base(&bob));
    let pending = tokio::spawn(async move { alice_client.post(url).json(&offer).send().await });

    // Wait until bob has the session.
    let mut sid = None;
    for _ in 0..100 {
        let v: Value = bob.invoke("session.list", json!({})).await.unwrap();
        if let Some(s) = v["sessions"].as_array().and_then(|a| a.first()) {
            sid = s["sid"].as_str().map(str::to_string);
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let sid = sid.expect("bob has the offered session");

    let resp = client_as(&mallory, &bob)
        .post(format!("{}/api/v1/session/{sid}/cancel", base(&bob)))
        .send()
        .await
        .expect("request reaches bob");
    assert_eq!(resp.status().as_u16(), 403, "a non-participant must not cancel the session");

    // The session is still alive: alice's own cancel with her certificate succeeds.
    let still = bob.invoke("session.list", json!({})).await.unwrap();
    assert!(still["sessions"].as_array().map(|a| !a.is_empty()).unwrap_or(false), "session survived");

    let ok = client_as(&alice, &bob)
        .post(format!("{}/api/v1/session/{sid}/cancel", base(&bob)))
        .send()
        .await
        .expect("request reaches bob");
    assert_eq!(ok.status().as_u16(), 204, "the session's own peer may cancel it");

    pending.abort();
    alice.stop().await;
    mallory.stop().await;
    bob.stop().await;
}
