//! Shared types for DropGrab: control API command and event names (dropgrab-app-design.md
//! section 3.5), stable error codes (section 14.1), and peer-protocol wire types (section 4.4).
//!
//! This crate depends on nothing internal, per the dependency rule in section 17.1.

use serde::{Deserialize, Serialize};

/// Version reported by `backend.info` and checked by clients.
pub const API_VERSION: &str = "1.0";

/// Peer protocol version (section 4.4.2 `protocolVersion`).
pub const PROTOCOL_VERSION: u32 = 1;

/// Every command in section 3.5 (`COMMANDS`), followed by the additive extensions this backend
/// exposes (`backend.*`/`pair.*` are in 3.5; nothing additional is needed for commands).
pub const COMMANDS: &[&str] = &[
    // Devices
    "devices.list",
    "devices.trust",
    "devices.block",
    "devices.forget",
    "devices.set_allowance",
    "visibility.set",
    // Transfer
    "session.send",
    "session.respond",
    "session.pause",
    "session.resume",
    "session.cancel",
    "session.history",
    "text.send",
    "pair.qr_create",
    "pair.qr_scan",
    // Downloads
    "downloads.add",
    "downloads.probe",
    "downloads.pause",
    "downloads.resume",
    "downloads.cancel",
    "downloads.reorder",
    "downloads.set_priority",
    "downloads.list",
    // Media
    "extractor.resolve",
    "extractor.update",
    "extractor.sites",
    // Library
    "library.query",
    "library.get",
    "library.delete",
    "library.move",
    "library.duplicates",
    // Remote
    "remote.request_download",
    "remote.list_requests",
    // Web-share
    "webshare.start",
    "webshare.stop",
    // System
    "settings.get",
    "settings.set",
    "diagnostics.run",
    "logs.export",
    "backend.info",
    "backend.shutdown",
];

/// Every event named in section 3.5 (`EVENTS`).
pub const EVENTS: &[&str] = &[
    "peer.up",
    "peer.down",
    "session.offered",
    "session.state",
    "session.progress",
    "session.finished",
    "download.state",
    "download.progress",
    "download.finished",
    "library.changed",
    "device.changed",
    "notification.request",
    "error",
    "resync",
];

/// Additive events used by the UI mock (`clients/web/js/mock/backend.js`). Section 3.5 says the
/// schema changes additively, so these are part of API version 1.0.
pub const EXTENSION_EVENTS: &[&str] = &[
    "settings.changed",
    "pair.updated",
    "webshare.started",
    "webshare.stopped",
];

pub fn is_known_command(method: &str) -> bool {
    COMMANDS.contains(&method)
}

/// Stable error codes. The string form is what clients see in `error.code`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    // Section 14.1
    #[serde(rename = "NET_UNREACHABLE")]
    NetUnreachable,
    #[serde(rename = "TLS_MISMATCH")]
    TlsMismatch,
    #[serde(rename = "DECLINED")]
    Declined,
    #[serde(rename = "TIMEOUT")]
    Timeout,
    #[serde(rename = "NO_SPACE")]
    NoSpace,
    #[serde(rename = "SRC_CHANGED")]
    SrcChanged,
    #[serde(rename = "RANGE_UNSUPPORTED")]
    RangeUnsupported,
    #[serde(rename = "EXTRACT_FAILED")]
    ExtractFailed,
    #[serde(rename = "DRM_PROTECTED")]
    DrmProtected,
    #[serde(rename = "AUTH_REQUIRED")]
    AuthRequired,
    #[serde(rename = "HASH_MISMATCH")]
    HashMismatch,
    #[serde(rename = "PERMISSION")]
    Permission,
    // API boundary codes
    #[serde(rename = "INVALID_PARAMS")]
    InvalidParams,
    #[serde(rename = "UNKNOWN_METHOD")]
    UnknownMethod,
    #[serde(rename = "UNKNOWN_DEVICE")]
    UnknownDevice,
    #[serde(rename = "UNKNOWN_SESSION")]
    UnknownSession,
    #[serde(rename = "UNKNOWN_JOB")]
    UnknownJob,
    #[serde(rename = "UNKNOWN_ITEM")]
    UnknownItem,
    #[serde(rename = "BLOCKED")]
    Blocked,
    #[serde(rename = "BUSY")]
    Busy,
    #[serde(rename = "PAIR_FAILED")]
    PairFailed,
    #[serde(rename = "NOT_ALLOWED")]
    NotAllowed,
    #[serde(rename = "UNAUTHORIZED")]
    Unauthorized,
    #[serde(rename = "FORBIDDEN")]
    Forbidden,
    #[serde(rename = "PAYLOAD_TOO_LARGE")]
    PayloadTooLarge,
    #[serde(rename = "RATE_LIMITED")]
    RateLimited,
    #[serde(rename = "NOT_IMPLEMENTED")]
    NotImplemented,
    #[serde(rename = "INTERNAL")]
    Internal,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        use ErrorCode::*;
        match self {
            NetUnreachable => "NET_UNREACHABLE",
            TlsMismatch => "TLS_MISMATCH",
            Declined => "DECLINED",
            Timeout => "TIMEOUT",
            NoSpace => "NO_SPACE",
            SrcChanged => "SRC_CHANGED",
            RangeUnsupported => "RANGE_UNSUPPORTED",
            ExtractFailed => "EXTRACT_FAILED",
            DrmProtected => "DRM_PROTECTED",
            AuthRequired => "AUTH_REQUIRED",
            HashMismatch => "HASH_MISMATCH",
            Permission => "PERMISSION",
            InvalidParams => "INVALID_PARAMS",
            UnknownMethod => "UNKNOWN_METHOD",
            UnknownDevice => "UNKNOWN_DEVICE",
            UnknownSession => "UNKNOWN_SESSION",
            UnknownJob => "UNKNOWN_JOB",
            UnknownItem => "UNKNOWN_ITEM",
            Blocked => "BLOCKED",
            Busy => "BUSY",
            PairFailed => "PAIR_FAILED",
            NotAllowed => "NOT_ALLOWED",
            Unauthorized => "UNAUTHORIZED",
            Forbidden => "FORBIDDEN",
            PayloadTooLarge => "PAYLOAD_TOO_LARGE",
            RateLimited => "RATE_LIMITED",
            NotImplemented => "NOT_IMPLEMENTED",
            Internal => "INTERNAL",
        }
    }
}

/// Error returned by a command. Serialized as `{"code": "...", "message": "..."}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code: code.as_str().to_string(), message: message.into() }
    }
}

/// JSON-RPC style request (section 3.5 shape).
#[derive(Debug, Clone, Deserialize)]
pub struct RpcRequest {
    pub id: serde_json::Value,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// JSON-RPC style response: exactly one of `result` or `error`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

/// Event envelope (section 3.5): `{ "seq", "type", "data" }`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub seq: u64,
    #[serde(rename = "type")]
    pub kind: String,
    pub data: serde_json::Value,
}

/// Peer-protocol identity block (section 4.4.3 `sender`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Sender {
    #[serde(rename = "deviceId")]
    pub device_id: String,
    pub alias: String,
    #[serde(rename = "deviceType")]
    pub device_type: String,
    pub fingerprint: String,
}

/// Discovery announcement (section 4.2 multicast, under 512 bytes).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Announce {
    #[serde(rename = "deviceId")]
    pub device_id: String,
    pub alias: String,
    #[serde(rename = "deviceType")]
    pub device_type: String,
    pub fingerprint: String,
    pub port: u16,
    #[serde(rename = "protocolVersion")]
    pub protocol_version: u32,
    pub capabilities: Vec<String>,
    pub announce: bool,
}

/// `GET /api/v1/info` response (section 4.4.2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InfoResponse {
    #[serde(rename = "deviceId")]
    pub device_id: String,
    pub alias: String,
    #[serde(rename = "deviceType")]
    pub device_type: String,
    pub os: String,
    #[serde(rename = "appVersion")]
    pub app_version: String,
    #[serde(rename = "protocolVersion")]
    pub protocol_version: u32,
    pub capabilities: Vec<String>,
    #[serde(rename = "maxParallelStreams")]
    pub max_parallel_streams: u32,
    pub fingerprint: String,
}

/// One file in an offer (section 4.4.3).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileMeta {
    pub fid: String,
    pub name: String,
    pub size: u64,
    pub mime: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified: Option<u64>,
}

/// `POST /api/v1/session/offer` request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OfferRequest {
    pub sender: Sender,
    pub files: Vec<FileMeta>,
    #[serde(rename = "totalSize")]
    pub total_size: u64,
}

/// Per-file grant in an accepted offer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileGrant {
    pub token: String,
    #[serde(rename = "resumeFrom")]
    pub resume_from: u64,
}

/// Offer response. `accepted` is always present; the other fields only when accepted/declined.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OfferResponse {
    pub accepted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<std::collections::BTreeMap<String, FileGrant>>,
    #[serde(rename = "chunkSize", skip_serializing_if = "Option::is_none")]
    pub chunk_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streams: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// `GET .../file/{fid}/status` response: byte ranges durably stored (inclusive start, exclusive end).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatusResponse {
    pub ranges: Vec<[u64; 2]>,
    pub size: u64,
}

/// `POST .../file/{fid}/complete` request body.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompleteRequest {
    pub sha256: String,
}

/// `POST /api/v1/text` request (section 4.9, up to 64 KiB).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TextRequest {
    pub sender: Sender,
    pub text: String,
}

/// `POST /api/v1/download-request` request (section 6.3).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DownloadRequest {
    pub sender: Sender,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// `POST /api/v1/pair` request: the one-time PIN from the QR payload (section 4.2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairRequest {
    pub sender: Sender,
    pub pin: String,
}

/// `POST /api/v1/pair` response. `verifyCode` is the 4-digit code both screens must show (section 4.3).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairResponse {
    #[serde(rename = "deviceId")]
    pub device_id: String,
    pub alias: String,
    pub fingerprint: String,
    #[serde(rename = "verifyCode")]
    pub verify_code: String,
}

/// Parsed `dropgrab://pair?v=1&id=..&fp=..&ip=..&port=..&pin=..` payload (section 4.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingUri {
    pub version: u32,
    pub device_id: String,
    pub fingerprint: String,
    pub ip: String,
    pub port: u16,
    pub pin: String,
}

impl PairingUri {
    pub fn parse(input: &str) -> Result<Self, String> {
        let rest = input
            .strip_prefix("dropgrab://pair?")
            .ok_or_else(|| "not a dropgrab://pair payload".to_string())?;
        let mut version = None;
        let mut device_id = None;
        let mut fingerprint = None;
        let mut ip = None;
        let mut port = None;
        let mut pin = None;
        for pair in rest.split('&') {
            let (k, v) = pair.split_once('=').ok_or_else(|| "malformed query".to_string())?;
            match k {
                "v" => version = v.parse::<u32>().ok(),
                "id" => device_id = Some(v.to_string()),
                "fp" => fingerprint = Some(v.to_string()),
                "ip" => ip = Some(v.to_string()),
                "port" => port = v.parse::<u16>().ok(),
                "pin" => pin = Some(v.to_string()),
                _ => {}
            }
        }
        let uri = PairingUri {
            version: version.ok_or("missing v")?,
            device_id: device_id.ok_or("missing id")?,
            fingerprint: fingerprint.ok_or("missing fp")?,
            ip: ip.ok_or("missing ip")?,
            port: port.ok_or("missing port")?,
            pin: pin.ok_or("missing pin")?,
        };
        if uri.version != 1 {
            return Err(format!("unsupported pairing version {}", uri.version));
        }
        if uri.pin.len() != 6 || !uri.pin.bytes().all(|b| b.is_ascii_digit()) {
            return Err("pin must be 6 digits".into());
        }
        Ok(uri)
    }

    pub fn format(&self) -> String {
        format!(
            "dropgrab://pair?v={}&id={}&fp={}&ip={}&port={}&pin={}",
            self.version, self.device_id, self.fingerprint, self.ip, self.port, self.pin
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_design_command_is_listed() {
        // The 43 names in dropgrab-app-design.md section 3.5.
        let design = [
            "devices.list", "devices.trust", "devices.block", "devices.forget",
            "devices.set_allowance", "visibility.set", "session.send", "session.respond",
            "session.pause", "session.resume", "session.cancel", "session.history", "text.send",
            "pair.qr_create", "pair.qr_scan", "downloads.add", "downloads.probe",
            "downloads.pause", "downloads.resume", "downloads.cancel", "downloads.reorder",
            "downloads.set_priority", "downloads.list", "extractor.resolve", "extractor.update",
            "extractor.sites", "library.query", "library.get", "library.delete", "library.move",
            "library.duplicates", "remote.request_download", "remote.list_requests",
            "webshare.start", "webshare.stop", "settings.get", "settings.set",
            "diagnostics.run", "logs.export", "backend.info", "backend.shutdown",
        ];
        assert_eq!(design.len(), 43);
        for name in design {
            assert!(COMMANDS.contains(&name), "missing command {name}");
        }
        assert_eq!(COMMANDS.len(), design.len());
    }

    #[test]
    fn every_design_event_is_listed() {
        let design = [
            "peer.up", "peer.down", "session.offered", "session.state", "session.progress",
            "session.finished", "download.state", "download.progress", "download.finished",
            "library.changed", "device.changed", "notification.request", "error", "resync",
        ];
        for name in design {
            assert!(EVENTS.contains(&name), "missing event {name}");
        }
    }

    #[test]
    fn pairing_uri_round_trip() {
        let uri = PairingUri {
            version: 1,
            device_id: "abc".into(),
            fingerprint: "ff00".into(),
            ip: "192.168.1.9".into(),
            port: 53318,
            pin: "482917".into(),
        };
        assert_eq!(PairingUri::parse(&uri.format()).unwrap(), uri);
        assert!(PairingUri::parse("dropgrab://pair?v=1&id=a&fp=b&ip=c&port=1&pin=12").is_err());
    }
}
