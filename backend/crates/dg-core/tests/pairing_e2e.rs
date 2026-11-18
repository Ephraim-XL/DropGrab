//! Pairing PIN lockout (section 4.3): a per-pairing-session failed-attempt counter. Five wrong
//! PINs lock that session and answer 429; a new `pair.qr_create` starts a fresh session, so one
//! attacker cannot lock the legitimate pairing permanently.

use std::path::PathBuf;

use dg_core::{discovery, engine::Config, Backend};
use dg_types::{PairRequest, Sender};
use serde_json::{json, Value};

struct TestDir(PathBuf);
impl TestDir {
    fn new(tag: &str) -> TestDir {
        let p = std::env::temp_dir().join(format!("dg-pair-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).ok();
        TestDir(p)
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn backend(tag: &str, dir: &TestDir) -> Backend {
    let mut cfg = Config::desktop(dir.0.join(format!("{tag}-data")), dir.0.join("downloads"));
    cfg.peer_port = 0;
    cfg.discovery = false;
    cfg.device_name = "Pair Test".into();
    Backend::start(cfg).await.expect("backend starts")
}

fn sender() -> Sender {
    Sender {
        device_id: "peer-phone-1".into(),
        alias: "Phone".into(),
        device_type: "phone".into(),
        fingerprint: "ab".repeat(32),
    }
}

async fn new_pin(b: &Backend) -> String {
    let v: Value = b.invoke("pair.qr_create", json!({})).await.expect("qr_create");
    v["pin"].as_str().expect("pin in qr_create reply").to_string()
}

fn wrong(pin: &str) -> String {
    // Same shape, different digits.
    let first = pin.chars().next().unwrap();
    let other = if first == '9' { '0' } else { '9' };
    format!("{other}{}", &pin[1..])
}

fn attempt(b: &Backend, pin: &str) -> Result<dg_types::PairResponse, (u16, String)> {
    discovery::handle_pair(&b.engine, PairRequest { sender: sender(), pin: pin.to_string() })
}

#[tokio::test]
async fn five_wrong_pins_lock_the_session_and_a_new_code_resets_it() {
    let dir = TestDir::new("lock");
    let b = backend("l", &dir).await;
    let pin = new_pin(&b).await;
    let bad = wrong(&pin);
    assert_ne!(pin, bad);

    for i in 0..4 {
        let e = attempt(&b, &bad).unwrap_err();
        assert_eq!(e.0, 403, "attempt {} should be a plain rejection, got {e:?}", i + 1);
    }
    // Fifth wrong PIN: locked.
    let e = attempt(&b, &bad).unwrap_err();
    assert_eq!(e.0, 429, "fifth wrong PIN must answer 429, got {e:?}");
    // The session stays locked, so even the real PIN is refused (no silent unlock).
    let e = attempt(&b, &pin).unwrap_err();
    assert_eq!(e.0, 429, "locked session must refuse the correct PIN too, got {e:?}");

    // A new pairing code is a new session with its own counter.
    let pin2 = new_pin(&b).await;
    let ok = attempt(&b, &pin2).expect("a fresh session accepts its own PIN");
    assert_eq!(ok.verify_code.len(), 4, "verification code is 4 digits");
    b.stop().await;
}

#[tokio::test]
async fn a_few_wrong_pins_do_not_lock_the_session() {
    let dir = TestDir::new("under");
    let b = backend("u", &dir).await;
    let pin = new_pin(&b).await;
    let bad = wrong(&pin);

    for _ in 0..(discovery::MAX_PAIR_ATTEMPTS - 1) {
        assert_eq!(attempt(&b, &bad).unwrap_err().0, 403);
    }
    let ok = attempt(&b, &pin).expect("correct PIN below the limit is accepted");
    assert_eq!(ok.verify_code.len(), 4);
    b.stop().await;
}
