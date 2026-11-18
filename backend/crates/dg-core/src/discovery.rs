//! Discovery (section 4.2): multicast announce on 239.255.42.99:53318, unicast replies, peer
//! table with stale/remove sweeps. QR pairing (4.2, 4.3) lives here too because it creates the
//! first trusted contact when discovery fails.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::Ordering;
use std::time::Duration;

use dg_types::{Announce, ErrorCode, PairRequest, PairResponse, PairingUri};
use serde_json::{json, Value};
use tokio::net::UdpSocket;

use crate::engine::{err, internal, lock, Engine, PairingState, Peer, Res};
use crate::util::now_ms;

pub fn announce_for(engine: &Engine, announce: bool) -> Announce {
    Announce {
        device_id: engine.identity.device_id.clone(),
        alias: engine.device_name(),
        device_type: engine.cfg.device_type.clone(),
        fingerprint: engine.identity.fingerprint.clone(),
        port: engine.peer_port,
        protocol_version: dg_types::PROTOCOL_VERSION,
        capabilities: engine.info().capabilities,
        announce,
    }
}

/// Starts the multicast listener and the periodic announcer. Failure to join the group is not
/// fatal: the QR path still works (section 10.3, "reduced capability").
pub fn spawn(engine: &Engine) {
    let e1 = engine.clone();
    engine.spawn(async move {
        if let Err(msg) = listen_loop(e1.clone()).await {
            tracing::warn!("multicast discovery disabled: {msg}");
            e1.emit(
                "error",
                json!({ "code": "NET_UNREACHABLE", "message": "multicast discovery unavailable", "detail": msg }),
            );
        }
    });
    let e2 = engine.clone();
    engine.spawn(async move {
        // Three announcements 200 ms apart on startup (section 4.2).
        for _ in 0..3 {
            let _ = send_announce(&e2, true).await;
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        let mut ticker = tokio::time::interval(e2.cfg.discovery_interval);
        ticker.tick().await;
        loop {
            ticker.tick().await;
            if e2.stopping.load(Ordering::SeqCst) {
                break;
            }
            if e2.visibility() != "hidden" {
                let _ = send_announce(&e2, true).await;
            }
            e2.sweep_peers();
        }
    });
}

async fn bind_multicast(engine: &Engine) -> Result<UdpSocket, String> {
    let sock = socket2::Socket::new(socket2::Domain::IPV4, socket2::Type::DGRAM, Some(socket2::Protocol::UDP))
        .map_err(|e| e.to_string())?;
    sock.set_reuse_address(true).map_err(|e| e.to_string())?;
    sock.set_nonblocking(true).map_err(|e| e.to_string())?;
    let bind: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), engine.cfg.discovery_port);
    sock.bind(&bind.into()).map_err(|e| format!("bind {bind}: {e}"))?;
    sock.join_multicast_v4(&engine.cfg.multicast_group, &Ipv4Addr::UNSPECIFIED)
        .map_err(|e| format!("join {}: {e}", engine.cfg.multicast_group))?;
    sock.set_multicast_loop_v4(true).map_err(|e| e.to_string())?;
    let std_sock: std::net::UdpSocket = sock.into();
    UdpSocket::from_std(std_sock).map_err(|e| e.to_string())
}

async fn listen_loop(engine: Engine) -> Result<(), String> {
    let sock = bind_multicast(&engine).await?;
    engine.multicast_joined.store(true, Ordering::SeqCst);
    let mut buf = vec![0u8; 2048];
    loop {
        if engine.stopping.load(Ordering::SeqCst) {
            return Ok(());
        }
        let (n, from) = match sock.recv_from(&mut buf).await {
            Ok(v) => v,
            Err(e) => return Err(e.to_string()),
        };
        if n > 512 {
            continue;
        }
        let msg: Announce = match serde_json::from_slice(&buf[..n]) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if msg.protocol_version != dg_types::PROTOCOL_VERSION || msg.device_id == engine.identity.device_id {
            continue;
        }
        if engine.visibility() == "hidden" {
            continue;
        }
        if engine.is_blocked(&msg.device_id, &msg.fingerprint) {
            continue;
        }
        let peer = Peer {
            device_id: msg.device_id.clone(),
            alias: msg.alias.clone(),
            device_type: msg.device_type.clone(),
            os: String::new(),
            fingerprint: msg.fingerprint.clone(),
            ip: from.ip(),
            port: msg.port,
            last_seen_ms: now_ms(),
            link: "same network".into(),
        };
        engine.note_peer(peer);
        if msg.announce {
            // Reply unicast so the sender learns about us without a reply storm (section 4.2).
            let reply = announce_for(&engine, false);
            if let Ok(bytes) = serde_json::to_vec(&reply) {
                let _ = sock.send_to(&bytes, from).await;
            }
        }
    }
}

async fn send_announce(engine: &Engine, announce: bool) -> Result<(), String> {
    if !engine.multicast_joined.load(Ordering::SeqCst) && engine.cfg.discovery_port == 0 {
        return Ok(());
    }
    let sock = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| e.to_string())?;
    let _ = sock.set_multicast_ttl_v4(1);
    let msg = announce_for(engine, announce);
    let bytes = serde_json::to_vec(&msg).map_err(|e| e.to_string())?;
    let target = SocketAddr::new(IpAddr::V4(engine.cfg.multicast_group), engine.cfg.discovery_port);
    sock.send_to(&bytes, target).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Manual refresh (`devices.list` does not block on it). Used by tests and the daemon.
pub async fn announce_now(engine: &Engine) -> Result<(), String> {
    send_announce(engine, true).await
}

/* ---------- pairing (QR) ---------- */

/// `pair.qr_create`: a one-time 6-digit PIN valid for 2 minutes (section 4.2).
pub fn qr_create(engine: &Engine) -> Res<Value> {
    let pin = format!("{:06}", rand::random::<u32>() % 1_000_000);
    let expires = now_ms() + engine.cfg.pair_ttl.as_millis() as i64;
    let ip = engine.advertise_ip();
    let uri = PairingUri {
        version: 1,
        device_id: engine.identity.device_id.clone(),
        fingerprint: engine.identity.fingerprint.clone(),
        ip: ip.to_string(),
        port: engine.peer_port,
        pin: pin.clone(),
    }
    .format();
    let code = format!("{} {}", &pin[..3], &pin[3..]);
    *lock(&engine.pairing) = Some(PairingState { pin: pin.clone(), expires_ms: expires, used: false, failed_pins: 0, uri: uri.clone() });
    engine.emit("pair.updated", json!({ "code": code, "expiresAt": expires }));
    Ok(json!({
        "code": code,
        "pin": pin,
        "expiresAt": expires,
        "uri": uri,
        "fingerprint": crate::util::display_fingerprint(&engine.identity.fingerprint),
    }))
}

/// `pair.qr_scan {uri}`: the sender side. Connects to the address in the QR, pins the
/// fingerprint from it, and returns the verification code for both screens.
pub async fn qr_scan(engine: &Engine, params: &Value) -> Res<Value> {
    let raw = params
        .get("uri")
        .and_then(Value::as_str)
        .ok_or_else(|| err(ErrorCode::InvalidParams, "uri is required"))?;
    let uri = PairingUri::parse(raw).map_err(|m| err(ErrorCode::InvalidParams, m))?;
    let ip: IpAddr = uri.ip.parse().map_err(|_| err(ErrorCode::InvalidParams, "ip is not an address"))?;
    if uri.device_id == engine.identity.device_id {
        return Err(err(ErrorCode::InvalidParams, "this QR code is from this device"));
    }
    let client = crate::identity::mtls_client(&engine.identity, &uri.fingerprint, Duration::from_secs(10)).map_err(internal)?;
    let url = match ip {
        IpAddr::V4(v4) => format!("https://{v4}:{}/api/v1/pair", uri.port),
        IpAddr::V6(v6) => format!("https://[{v6}]:{}/api/v1/pair", uri.port),
    };
    let body = PairRequest { sender: engine.self_sender(), pin: uri.pin.clone() };
    let resp = client.post(url).json(&body).send().await.map_err(|e| {
        if e.to_string().contains("TLS_MISMATCH") {
            err(ErrorCode::TlsMismatch, "the device's certificate does not match the QR code")
        } else {
            err(ErrorCode::NetUnreachable, "could not reach the device for pairing")
        }
    })?;
    if !resp.status().is_success() {
        return Err(err(ErrorCode::PairFailed, "the device rejected the pairing code"));
    }
    let answer: PairResponse = resp.json().await.map_err(|_| err(ErrorCode::PairFailed, "bad pairing reply"))?;
    if answer.fingerprint != uri.fingerprint {
        return Err(err(ErrorCode::TlsMismatch, "fingerprint mismatch"));
    }
    engine.note_peer(Peer {
        device_id: answer.device_id.clone(),
        alias: answer.alias.clone(),
        device_type: "unknown".into(),
        os: String::new(),
        fingerprint: answer.fingerprint.clone(),
        ip,
        port: uri.port,
        last_seen_ms: now_ms(),
        link: "paired".into(),
    });
    Ok(json!({
        "deviceId": answer.device_id,
        "name": answer.alias,
        "verifyCode": answer.verify_code,
        "fingerprint": crate::util::display_fingerprint(&answer.fingerprint),
    }))
}

/// Wrong PINs allowed on one pairing session before it locks (mirrors `webshare::MAX_PIN_ATTEMPTS`).
pub const MAX_PAIR_ATTEMPTS: u32 = 5;

/// `POST /api/v1/pair` (receiver side): checks the PIN once, records the sender as seen, and
/// returns the verification code. Trust is set later by the user (section 4.3 step 3).
///
/// The failed-attempt counter belongs to the pairing session, so five wrong guesses lock only
/// that code: the user gets a fresh one with `pair.qr_create`.
pub fn handle_pair(engine: &Engine, req: PairRequest) -> Result<PairResponse, (u16, String)> {
    let mut guard = lock(&engine.pairing);
    let state = guard.as_mut().ok_or((403, "no pairing code is active".to_string()))?;
    if now_ms() > state.expires_ms {
        *guard = None;
        return Err((410, "pairing code expired".into()));
    }
    if state.failed_pins >= MAX_PAIR_ATTEMPTS {
        return Err((429, "too many wrong pairing codes".into()));
    }
    if state.used {
        return Err((403, "wrong or used pairing code".into()));
    }
    if state.pin != req.pin {
        state.failed_pins += 1;
        if state.failed_pins >= MAX_PAIR_ATTEMPTS {
            return Err((429, "too many wrong pairing codes".into()));
        }
        return Err((403, "wrong or used pairing code".into()));
    }
    state.used = true;
    let fp = req.sender.fingerprint.clone();
    let id = req.sender.device_id.clone();
    let alias = req.sender.alias.clone();
    drop(guard);
    let _ = engine.upsert_device(&id, &alias, &req.sender.device_type, "", &fp);
    engine.emit("device.changed", json!({ "deviceId": id, "trust": "seen" }));
    let code = crate::identity::verify_code(&engine.identity.fingerprint, &fp);
    Ok(PairResponse {
        device_id: engine.identity.device_id.clone(),
        alias: engine.device_name(),
        fingerprint: engine.identity.fingerprint.clone(),
        verify_code: code,
    })
}
