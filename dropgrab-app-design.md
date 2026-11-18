# DropGrab: Download Anything, Send Anywhere

**Product and technical design document, v1.1 (Rust backend architecture)**
Working name: DropGrab (placeholder).
Platforms: Android, iOS, Windows, macOS, Linux, plus a browser extension.

---

## Table of contents

1. Product overview
2. Feature specification
3. System architecture (Rust backend)
4. Transfer subsystem
5. Download subsystem
6. Integration between download and transfer
7. Library and storage
8. Platform specifics
9. User experience design
10. Security and privacy
11. Performance targets
12. Background behavior and notifications
13. Settings
14. Error handling and diagnostics
15. Testing strategy
16. Legal, policy, and distribution
17. Project structure and roadmap
18. Risks and open questions

---

## 1. Product overview

### 1.1 Vision

One app, installed on phones and computers, that does two jobs well:

1. **Get it.** Download anything from the internet: direct files, videos, audio, streams, and torrents. Fast, resumable, queued, and organized.
2. **Move it.** Send anything to any nearby device (Android, iOS, Windows, macOS, Linux) with no internet connection, no account, and no cable.

The two halves feed each other. A finished download can be sent to another device in one tap, and anything received lands in the same library as anything downloaded. A phone can hand a link to a paired computer to download on a faster connection, then pull the result back.

### 1.2 Goals

- G1: Transfer files between any two supported devices, across operating systems and across mobile and desktop.
- G2: Reach the maximum speed the available link allows, including router-free device-to-device links.
- G3: Download files, video, audio, and streams reliably, with resume after any interruption.
- G4: Detect downloadable media automatically in a built-in browser and in desktop browsers.
- G5: Require no account, no cloud, and no ads. Everything works locally.
- G6: Keep a single library for downloaded and received items.

### 1.3 Non-goals

- Circumventing DRM-protected content.
- Hosting, indexing, or recommending content.
- Cloud storage or syncing through a server the app operates.
- Social features, chat, or user profiles.
- Remote (over the internet) transfers between devices in v1. Transfers are nearby or same-network. Remote queueing of downloads between paired devices is in scope (section 6.3).

### 1.4 Target users

| Persona | Need | Key features |
|---|---|---|
| Student | Share notes, slides, and videos between phone and laptop; save lectures | Send, library, video download |
| Content collector | Save videos and music for offline use | Browser sniffing, format picker, playlists |
| Small team or family | Move large files between mixed devices (Android, iPhone, Windows) | Cross-platform send, web-share |
| Low-connectivity user | Download on one device with a good connection, redistribute to others | Remote queue, hotspot transfer, scheduling |

### 1.5 Success metrics

| Metric | Target |
|---|---|
| Time from opening the app to first successful transfer (two devices on same Wi-Fi) | Under 30 seconds |
| Same-network transfer throughput (large file) | At least 70% of measured link throughput |
| Resume success after forced app kill mid-download | 99% or better |
| Download completion rate for supported direct URLs | 98% or better |
| Crash-free sessions | 99.5% or better |
| Cold start to interactive | Under 2 seconds on a mid-range phone |

---

## 2. Feature specification

Priority key: **P0** launch (MVP), **P1** first major update, **P2** later.

### 2.1 Transfer features

| ID | Feature | Pri | Description |
|---|---|---|---|
| T1 | Nearby device discovery | P0 | Devices on the same network appear automatically with name, type, and icon |
| T2 | Send files and folders | P0 | Any file type, any size, multiple items in one session |
| T3 | Send text and clipboard | P0 | Send a string or link; receiver can copy or open |
| T4 | Receive with approval | P0 | Incoming sessions show sender, file list, total size; accept or decline |
| T5 | QR and code pairing | P0 | Connect when automatic discovery is blocked |
| T6 | Progress, pause, cancel | P0 | Per-file and per-session progress, speed, and ETA |
| T7 | Resume interrupted transfers | P0 | Continue from the last confirmed byte |
| T8 | Trusted devices | P1 | Pair once; later sends auto-accept if enabled |
| T9 | Fast link negotiation | P1 | Upgrade to a direct hotspot or peer-to-peer link when available |
| T10 | Low-power discovery | P1 | Bluetooth LE advertising to wake nearby devices |
| T11 | Web-share | P1 | Receiver opens a link in any browser; no install needed |
| T12 | Transfer history | P1 | Searchable log of past sessions |
| T13 | Folder sync (one-way push) | P2 | Mirror a chosen folder to a trusted device |
| T14 | Device migration | P2 | Move media, contacts export files, and documents to a new phone |
| T15 | Share-sheet integration | P0 | Send from any app through the operating system's share menu |

### 2.2 Download features

| ID | Feature | Pri | Description |
|---|---|---|---|
| D1 | Direct file download | P0 | HTTP/HTTPS with multi-connection acceleration |
| D2 | Pause, resume, retry | P0 | Survive network changes, restarts, and crashes |
| D3 | Queue and concurrency limits | P0 | Configurable parallel downloads |
| D4 | Speed limit and scheduler | P0 | Global limit; time windows; Wi-Fi-only rules |
| D5 | Clipboard link detection | P0 | Offer to download when a URL is copied |
| D6 | Video and audio extraction | P0 | Resolve a page URL to downloadable formats |
| D7 | Format and quality picker | P0 | Resolution, codec, bitrate, container; audio-only |
| D8 | In-app browser with media detection | P1 | Detects media requests while browsing |
| D9 | Stream download (HLS/DASH) | P1 | Non-DRM segmented streams assembled into one file |
| D10 | Browser extension | P1 | Captures downloads and videos in desktop browsers |
| D11 | Playlist and batch download | P1 | Select multiple items from a page or list |
| D12 | Post-processing | P1 | Mux, convert audio, embed subtitles and thumbnails |
| D13 | Torrent and magnet links | P2 | Optional module, off by default |
| D14 | Authenticated downloads | P1 | Use cookies or headers the user supplies for sites they log in to |
| D15 | Share-to-app intake | P1 | Send a link to the app from a browser share menu |
| D16 | Download rules | P2 | Auto-sort by type, rename by template, move to folders |

### 2.3 Integration and library features

| ID | Feature | Pri | Description |
|---|---|---|---|
| L1 | Unified library | P0 | Downloads and received items in one index |
| L2 | Send from library | P0 | Any library item can be sent to a device |
| L3 | Built-in previews | P0 | Images, audio, video, PDFs, text |
| L4 | Search and filters | P1 | By name, type, source, date, size |
| L5 | Remote download queue | P1 | Add a link on one paired device; another downloads it |
| L6 | Storage manager | P1 | Usage by type, duplicate finder, bulk delete |
| L7 | Collections | P2 | User-defined groups |

---

## 3. System architecture (Rust backend)

### 3.1 Principle: one Rust backend, thin clients

All product logic lives in a single **Rust backend** (the "engine"). Every user interface is a thin client: it renders state, sends commands, and subscribes to events. No UI contains protocol, download, crypto, or storage logic.

```
+--------------------------------------------------------------------------+
|                               CLIENTS (thin)                             |
|  Mobile UI (React Native)   Desktop UI (Tauri)   Browser extension   CLI |
|  Headless web UI (served by the backend)                                 |
+----------------------+-------------------------------+-------------------+
                       | commands (request/response)   |
                       | events (server push stream)   |
+----------------------v-------------------------------v-------------------+
|                         CONTROL API (dg-api)                             |
|   Desktop: local socket / named pipe     Mobile: in-process FFI          |
|   Same command and event schema on every transport                       |
+--------------------------------------------------------------------------+
|                     RUST BACKEND SERVICES (tokio)                        |
|  DiscoveryService   SessionService    DownloadService   LibraryService   |
|  DeviceService      WebShareService   ExtractorService  RuleEngine       |
|  SettingsService    SchedulerService  NotificationHub   UpdateService    |
+--------------------------------------------------------------------------+
|                          ENGINE CRATES                                   |
|  transport | crypto/trust | segments | streams | media bridge | storage  |
+--------------------------------------------------------------------------+
|                 PLATFORM ADAPTER TRAITS (implemented per OS)             |
|  FileAccess | Radio | Notifications | Power | BackgroundTransfer | Share |
+--------------------------------------------------------------------------+
|                      SQLite (WAL) + file system                          |
+--------------------------------------------------------------------------+
```

Why this shape:

- One implementation of the protocol, the download engine, and the trust model for five operating systems, so a bug is fixed once.
- Memory safety and predictable performance for the hot paths (hashing, chunk I/O, segment scheduling).
- The UI can crash, restart, or be replaced without interrupting a transfer or download that the backend owns.
- A stable command/event schema makes a CLI, a headless web UI, and a browser extension possible with no extra backend work.

### 3.2 Deployment modes

| Mode | Where the backend runs | How the UI connects |
|---|---|---|
| Desktop daemon | `dropgrabd`, a separate background process started at login (tray or menu-bar icon) | Local socket (Unix domain socket, Windows named pipe) |
| Android | Shared library (`libdropgrab.so`) loaded inside a foreground service | In-process FFI bridge |
| iOS | Static library linked into the app | In-process FFI bridge |
| Headless server | `dropgrabd --headless` on a NAS, Raspberry Pi, home server, or VPS | Authenticated HTTPS web UI and the same control API over TLS |
| Command line | `dropgrab` CLI, a client of any running backend | Local socket or HTTPS |

The command and event schema is identical in all modes, so a UI written against one mode works against the others.

### 3.3 Technology decisions

| Concern | Decision | Rationale |
|---|---|---|
| Backend language | Rust (stable toolchain, fixed minimum supported version) | Safety, performance, one codebase for all targets |
| Async runtime | `tokio` (multi-threaded on desktop; bounded worker count on mobile) | Mature, supports sockets, timers, files, and cancellation |
| HTTP server (peer API, control API, web-share) | `axum` on `hyper` | Composable, async, strongly typed handlers |
| TLS | `rustls` with per-device self-signed certificates created by `rcgen`; custom certificate verifier that pins fingerprints | No dependency on a system TLS library or certificate authorities |
| HTTP client (downloads) | `hyper`-based client with `rustls`, HTTP/1.1 and HTTP/2, proxy support | One stack for TLS and connection pooling |
| Storage | SQLite through `rusqlite`, WAL mode, one writer task plus a small read pool | Reliable, embedded, no server |
| Serialization | `serde` with JSON for the peer protocol and control API | Debuggable, language-neutral |
| Type sharing with UIs | Generate TypeScript types from the Rust types (`specta` or `ts-rs`) and publish the schema | UI and backend cannot drift apart |
| Discovery | `socket2` for multicast sockets, `mdns-sd` for service announcements where useful, `btleplug` for Bluetooth LE on desktop, native BLE through the platform adapter on mobile | Cross-platform with escape hatches |
| Hashing | `sha2` (hardware accelerated), computed on a blocking worker pool | Overlaps with I/O |
| Compression | `zstd` for text-like payloads only | Fast, optional |
| Streams | `m3u8-rs` and `dash-mpd` for manifest parsing; own fragment scheduler | Manifest parsing is a security-sensitive area; use well-fuzzed parsers |
| Media muxing | ffmpeg-class muxer as a sidecar process on desktop; a linked, remux-only build on mobile, both behind a `Muxer` trait | Keeps the muxer replaceable and the licensing boundary clear |
| Extractor | Signed WebAssembly plugins run in a sandboxed runtime (`wasmtime` on desktop and Android; an interpreter-based runtime such as `wasmi` on iOS, where just-in-time code is not allowed) | Updatable without an app release, sandboxed, no native code downloaded |
| Update signing | Ed25519 signatures (`ed25519-dalek`) with a pinned public key | Prevents tampered updates |
| Logging and tracing | `tracing` with a redacting formatter | Structured, filterable, no URLs in logs by default |
| Errors | `thiserror` for library crates, a single stable error-code enum at the API boundary | Predictable user-facing codes |
| Mobile bridge | `uniffi`-generated Kotlin and Swift bindings | One interface definition, two platforms |
| Desktop shell | Tauri (UI only; it spawns or attaches to `dropgrabd`) | Small installer, UI independent of the engine |
| QR codes | `qrcode` crate | Pure Rust |

### 3.4 Backend internal architecture

**Service model.** Each service is a long-lived async task that owns its state exclusively. Other parts of the backend talk to it by sending a command message on a bounded channel; there is no shared mutable state between services.

```
UI  --command-->  dg-api  --route-->  Service (owns state)  --event-->  EventBus  --push-->  UI
```

| Service | Owns | Receives commands for | Emits events |
|---|---|---|---|
| DiscoveryService | Peer table, sockets, BLE scan | refresh, set visibility, scan QR | peer up, peer down, peer updated |
| DeviceService | Trusted, blocked, and recent device records | trust, block, forget, set allowances | device changed |
| SessionService | Active transfer sessions and the peer HTTPS server | offer, respond, pause, cancel, resume | session offered, state, progress, finished |
| DownloadService | Download jobs, segment engine, queue | add, pause, resume, cancel, reorder, retry | job state, progress, finished |
| ExtractorService | Plugin runtime, extractor cache, update state | resolve, update, list sites | resolve result, update status |
| SchedulerService | Schedules, speed limits, network rules | set schedule, set limits | window opened, window closed |
| LibraryService | Item index, previews, duplicate finder | query, delete, move, rename | library changed |
| WebShareService | Temporary web-share servers | start, stop | share started, share expired |
| RuleEngine | Naming and routing rules | create, update, evaluate | rule applied |
| SettingsService | Configuration | get, set | settings changed |
| NotificationHub | Notification policy | (internal) | notification requested |
| UpdateService | Backend and extractor updates | check, apply, roll back | update available, applied |

**Concurrency rules**

- Channels are **bounded**. A full channel applies backpressure to the sender instead of growing memory.
- Cancellation uses a **hierarchy of cancellation tokens**: backend, then service, then session or job, then individual segment or chunk. Cancelling a parent cancels all children promptly.
- CPU-heavy or blocking work (hashing, file allocation, directory walks, SQLite) runs on dedicated blocking pools, never on the async worker threads.
- Disk writes use positional writes at explicit offsets, so concurrent segments of one file need no locking.
- Buffers come from a pool (`bytes::BytesMut` slabs) with a global cap; streaming code never buffers a whole file.
- Services are supervised: if a service task panics or exits unexpectedly, the supervisor logs it, emits an `error` event, and restarts the service with backoff. Jobs in flight reload from SQLite.
- Progress events are **coalesced** in the backend (default 4 per second per item) so a fast transfer cannot flood the UI.

**Persistence**

- A single writer task serializes all SQLite writes in batched transactions; reads use a small pool.
- Download and session progress is checkpointed every 512 KiB or 2 seconds, so a crash loses at most that window.
- Schema migrations run at startup, are forward-only, and back up the database first.

### 3.5 Control API (UI to backend)

**Transports**

| Mode | Transport | Access control |
|---|---|---|
| Desktop | Unix domain socket (`$XDG_RUNTIME_DIR/dropgrab/api.sock` on Linux, `~/Library/Application Support/DropGrab/api.sock` on macOS) or Windows named pipe (`\\.\pipe\dropgrab-<user>`) | File or pipe permissions restricted to the current user; peer credentials checked |
| Desktop (optional) | Loopback TCP, port 53319, disabled by default | Per-install random token in an `Authorization` header; token file readable only by the user |
| Headless server | HTTPS on the LAN interface | Mandatory token or password, TLS, explicit opt-in |
| Mobile | In-process FFI | The app process boundary |

The control API is **never** bound to the LAN in desktop or mobile modes. It is separate from the peer protocol port.

**Shape**

- Commands: request/response in a JSON-RPC style (`{ "id", "method", "params" }` and `{ "id", "result" | "error" }`).
- Events: a push stream (WebSocket or server-sent events over the same connection family; a callback on mobile) delivering `{ "seq", "type", "data" }` items.
- Reconnect support: each event has a monotonically increasing sequence number. A reconnecting client sends the last `seq` it saw and receives the missed events, or a `resync` marker telling it to reload full state through query commands.
- Versioning: the backend reports `apiVersion`; clients declare the minimum they need. Changes are additive; breaking changes require a major version bump.

**Representative commands**

| Group | Commands |
|---|---|
| Devices | `devices.list`, `devices.trust`, `devices.block`, `devices.forget`, `devices.set_allowance`, `visibility.set` |
| Transfer | `session.send`, `session.respond`, `session.pause`, `session.resume`, `session.cancel`, `session.history`, `text.send`, `pair.qr_create`, `pair.qr_scan` |
| Downloads | `downloads.add`, `downloads.probe`, `downloads.pause`, `downloads.resume`, `downloads.cancel`, `downloads.reorder`, `downloads.set_priority`, `downloads.list` |
| Media | `extractor.resolve`, `extractor.update`, `extractor.sites` |
| Library | `library.query`, `library.get`, `library.delete`, `library.move`, `library.duplicates` |
| Remote | `remote.request_download`, `remote.list_requests` |
| Web-share | `webshare.start`, `webshare.stop` |
| System | `settings.get`, `settings.set`, `diagnostics.run`, `logs.export`, `backend.info`, `backend.shutdown` |

**Representative events**

`peer.up`, `peer.down`, `session.offered`, `session.state`, `session.progress`, `session.finished`, `download.state`, `download.progress`, `download.finished`, `library.changed`, `device.changed`, `notification.request`, `error`, `resync`.

**Type definitions (excerpt)**

```rust
#[derive(Serialize, Deserialize, TS)]
#[serde(tag = "method", content = "params")]
pub enum Command {
    #[serde(rename = "downloads.add")]
    DownloadsAdd { url: String, options: DownloadOptions },
    #[serde(rename = "session.send")]
    SessionSend { peer: DeviceId, items: Vec<ItemRef> },
    #[serde(rename = "session.respond")]
    SessionRespond { sid: SessionId, accept: bool, trust: bool },
    // ...
}

#[derive(Serialize, Deserialize, TS)]
#[serde(tag = "type", content = "data")]
pub enum Event {
    #[serde(rename = "session.progress")]
    SessionProgress { sid: SessionId, done: u64, total: u64, bps: u64, eta_s: Option<u32> },
    #[serde(rename = "download.state")]
    DownloadState { id: JobId, state: JobState, error: Option<ErrorCode> },
    // ...
}
```

**Local API security**

- The control API only listens on user-private transports (socket, pipe, loopback).
- Loopback TCP rejects any request carrying an `Origin` header not on an allowlist and sends no CORS headers, which blocks malicious web pages from driving the backend (including DNS-rebinding attempts); it also checks the `Host` header.
- Mutating commands require the token; read-only status may be allowed without it on the user-private socket.
- Rate limits and size limits apply to every command to protect against buggy or hostile local clients.

### 3.6 Platform adapter traits

The backend is portable because everything operating-system-specific sits behind traits that the backend defines. Desktop implements them in Rust; Android and iOS implement them in Kotlin and Swift and hand them to the backend at startup through callback interfaces.

```rust
pub trait PlatformAdapter: Send + Sync {
    fn files(&self) -> &dyn FileAccess;            // open, create, list, free space, permissions
    fn radio(&self) -> &dyn RadioControl;          // BLE scan/advertise, hotspot, peer-to-peer link
    fn notifications(&self) -> &dyn Notifier;      // post, update, cancel
    fn power(&self) -> &dyn PowerState;            // battery level, charging, metered network
    fn background(&self) -> &dyn BackgroundTransfer; // OS-managed downloads (iOS), keep-awake
    fn share(&self) -> &dyn ShareIntents;          // incoming share and open-with events
    fn network(&self) -> &dyn NetworkState;        // interfaces, SSID, Wi-Fi vs cellular
}
```

| Trait | Android | iOS | Desktop |
|---|---|---|---|
| FileAccess | Scoped storage through content URIs and file descriptors passed to Rust | Security-scoped bookmarks and app container paths | Native paths |
| RadioControl | Nearby-device APIs, local-only hotspot, peer-to-peer Wi-Fi | BLE (foreground), guided network join | Native stacks via crates, or OS tools |
| Notifications | Notification channels, foreground-service notification | Local notifications | Native notification centers |
| BackgroundTransfer | Foreground service keeps the backend alive | OS background URL sessions for eligible downloads | Not needed |
| Power and network | Battery and connectivity managers | Network path monitor, low-power mode | Power and network APIs |

File descriptors cross the boundary where possible, so Rust performs the I/O at full speed and the platform only grants access.

### 3.7 Mobile embedding

- **Lifecycle.** The UI calls `Backend.start(config, dataDir, adapter)` once per process. It returns a handle with `invoke(command)` and `subscribe(callback)`. `stop()` flushes state and shuts services down in order.
- **Android.** The backend lives in a foreground service. If the UI process is killed, the service keeps running until work is done. Rust threads are attached to the JVM only when they call back into Kotlin.
- **iOS.** The backend lives in the app process and runs while the app is foregrounded or has background time. Eligible direct downloads are handed to the OS background transfer service through `BackgroundTransfer`; the backend reconciles their state when the app returns. Segmented acceleration is used in the foreground only; the background path trades speed for continuity.
- **Panic safety.** Every FFI entry point wraps its body in `catch_unwind`; a panic becomes an `INTERNAL` error code and a supervisor restart of the affected service, never a process abort.
- **Threading.** The backend owns its own runtime; the UI thread never blocks on the backend. Calls are asynchronous with completion callbacks.
- **Size budget.** The stripped backend library (without the muxer) is budgeted at 12 MB or less per mobile architecture, using link-time optimization and size-optimized profiles for mobile builds.

### 3.8 Desktop daemon

- **Single instance.** `dropgrabd` takes an exclusive lock; a second launch connects to the first instead of starting another engine.
- **Autostart.** Optional, installed per user: a launch agent on macOS, a per-user systemd unit (or desktop autostart entry) on Linux, a per-user startup entry on Windows.
- **Shell relationship.** The Tauri UI attaches to a running daemon or starts it. Closing the UI leaves the daemon running; "Quit" from the tray stops it after active work finishes (or on confirmation).
- **Watchdog.** A tiny supervisor restarts the daemon after a crash and writes a crash report for opt-in submission.
- **Updates.** The daemon updates itself from signed packages; it waits for an idle moment and never replaces a binary during an active transfer.
- **Native messaging host.** `dropgrab-nmhost`, a small Rust binary, speaks the browser's native-messaging protocol on stdin/stdout and the control API to the daemon (section 5.12).

### 3.9 Headless server mode

An optional mode for people with an always-on machine (NAS, Raspberry Pi, mini PC, VPS).

- Start with `dropgrabd --headless`. The backend serves a web UI and the control API over HTTPS.
- It is off by default. Enabling it requires setting a password or token; the first start prints a one-time setup URL.
- Default binding is the local network; internet exposure is the user's choice and the documentation recommends a VPN or reverse proxy with its own authentication.
- Typical uses: an always-on download host that phones and laptops push links to (the remote download queue), and a shared receiver for a small office or household.
- The web UI uses the same schema and components as the desktop UI.

### 3.10 Rust engineering rules

- `#![forbid(unsafe_code)]` in all crates except the FFI crate and the platform shims, where every `unsafe` block carries a safety comment and is covered by tests (run under Miri where practical).
- Clippy with `unwrap_used`, `expect_used`, `indexing_slicing`, and `panic` denied in non-test code.
- Parsing of untrusted input (manifests, filenames, headers, peer messages) is isolated in small crates with fuzz targets (`cargo-fuzz`).
- `cargo-deny` for license and source policy, `cargo-audit` for advisories, and a lockfile committed for every binary.
- Reproducible release builds with pinned toolchains; each release attaches a software bill of materials.
- Public API of each crate documented; integration points covered by trait-based fakes so services can be tested without real sockets or disks.

### 3.11 Build targets

| Target group | Rust targets | Output |
|---|---|---|
| Android | `aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android` | `libdropgrab.so` plus generated Kotlin bindings, packaged in the app |
| iOS | `aarch64-apple-ios`, `aarch64-apple-ios-sim`, `x86_64-apple-ios` | XCFramework plus generated Swift bindings |
| Windows | `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc` | `dropgrabd.exe`, `dropgrab.exe`, `dropgrab-nmhost.exe` |
| macOS | `x86_64-apple-darwin`, `aarch64-apple-darwin` | Universal binaries |
| Linux | `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, plus `musl` variants for the headless build | Binaries and packages |

Continuous integration builds every target on each change, runs unit and integration tests on host targets, and runs the FFI smoke tests on emulators.

### 3.12 Core module responsibilities (summary)

- **Discovery:** finds peers; maintains a live peer table with last-seen time, addresses, and capabilities.
- **Transport:** sessions, chunked streams, resume, parallel connections, link negotiation.
- **Crypto and trust:** certificate generation, fingerprint verification, trusted-device store.
- **Segment engine:** range planning, segment scheduling, persistence of progress, retries.
- **Extractor host:** runs signed extractor plugins in a sandbox, supplies them with a restricted fetch function, returns normalized format lists.
- **Stream assembler:** fetches manifests, downloads fragments in parallel, hands them to the muxer.
- **Media muxer bridge:** combines video and audio, converts audio, embeds metadata.
- **Web-share server:** serves a temporary, PIN-protected page and endpoints.
- **Library and rules:** indexes files, applies naming and folder rules to completed items.

---

## 4. Transfer subsystem

### 4.1 Device identity

Every installation generates on first launch:

- `deviceId`: random 128-bit identifier (UUID v4).
- A self-signed X.509 certificate and private key (ECDSA P-256), stored in the platform keystore where available.
- `fingerprint`: SHA-256 of the certificate, shown as 8 groups of 4 hex characters.
- `alias`: user-editable display name, defaulting to a generated name such as "Swift Falcon".
- `deviceType`: phone, tablet, desktop, laptop, web.

### 4.2 Discovery

Several mechanisms run in parallel; results are merged into one peer table keyed by `deviceId`.

| Mechanism | Scope | When used |
|---|---|---|
| Multicast announce | Same subnet | Default on home and office networks |
| Subnet scan | Same subnet | When multicast is blocked; probes the info endpoint on neighbouring addresses with a short timeout |
| QR or code pairing | Any reachable network | When both of the above fail, or for one-time connections |
| Bluetooth LE advertise/scan | Within radio range | Wakes sleeping devices and exchanges connection hints |
| Hotspot mode | Direct | No shared network exists |

**Multicast announce**

- Group `239.255.42.99`, UDP port `53318`.
- Announce message (JSON, under 512 bytes): `deviceId`, `alias`, `deviceType`, `fingerprint`, `port`, `protocolVersion`, `capabilities[]`, `announce` (true on startup or when answering).
- On startup, send three announcements 200 ms apart. Devices that hear an announcement with `announce=true` reply with a unicast announcement to the sender (not multicast), which avoids reply storms.
- Peers not heard from in 30 seconds are marked stale; after 120 seconds they are removed unless pinned as trusted.

**Subnet scan**

- Run only if no peer has been found within 3 seconds of opening the Send screen, or on manual refresh.
- Probe `GET /api/v1/info` on every host in the local /24 at port `53318` with a 400 ms timeout, 64 in flight.
- Skip scanning on subnets larger than /22 and on networks the system classifies as public Wi-Fi unless the user enables it, to avoid network-abuse flags.

**QR pairing payload**

```
dropgrab://pair?v=1&id=<deviceId>&fp=<fingerprint>&ip=<ip>&port=<port>&pin=<6-digit>
```

The receiver shows the QR; the sender scans it, connects to the address in it, and checks that the presented certificate matches `fp`. The `pin` is a one-time code valid for 2 minutes, required for the first connection.

**Bluetooth LE**

- Advertises a service ID plus a 4-byte rotating token derived from `deviceId` (so devices cannot be tracked over time).
- A scanning device that sees a known token wakes the peer's network listener and obtains its current address through a short GATT read.
- Optional; the transfer itself never travels over BLE.

### 4.3 Trust and pairing

States of a remote device from the local point of view:

| State | Meaning | Behavior |
|---|---|---|
| Unknown | Never seen, or not saved | Every incoming session requires approval; fingerprint shown |
| Seen | Appeared in a previous session | Approval required; alias remembered |
| Trusted | User explicitly trusted after verifying | May auto-accept if enabled |
| Blocked | User blocked | Connections refused silently |

First-time trust flow:

1. Both screens show a 4-digit verification code derived from both fingerprints (hash of the sorted pair, reduced to 4 digits).
2. The user confirms the codes match on both devices.
3. Each device stores the other's fingerprint as Trusted.

A certificate change on a Trusted device downgrades it to Unknown and shows a warning.

### 4.4 Session protocol

All requests are HTTPS on the device's port. Clients verify the server certificate against the expected fingerprint (from discovery, QR, or the trusted list) rather than a certificate authority. Requests and responses use JSON unless noted.

#### 4.4.1 Endpoints

| Method and path | Direction | Purpose |
|---|---|---|
| `GET /api/v1/info` | any to any | Returns device info and capabilities |
| `POST /api/v1/announce` | any to any | Unicast form of the discovery announcement |
| `POST /api/v1/session/offer` | sender to receiver | Propose a transfer: file list, totals, sender identity |
| `POST /api/v1/session/{sid}/accept` | internal / receiver UI | Receiver decision (kept on the receiver; the response to `offer` carries it) |
| `PUT /api/v1/session/{sid}/file/{fid}` | sender to receiver | Send file bytes or a chunk range |
| `GET /api/v1/session/{sid}/file/{fid}/status` | sender to receiver | Receiver reports bytes durably stored; used for resume |
| `POST /api/v1/session/{sid}/file/{fid}/complete` | sender to receiver | Sender signals end of a file with final hash |
| `POST /api/v1/session/{sid}/cancel` | either | Abort the session |
| `POST /api/v1/text` | sender to receiver | Send a text or link payload |
| `POST /api/v1/download-request` | paired device | Ask this device to download a URL (remote queue) |

#### 4.4.2 Info response

```json
{
  "deviceId": "6f1c...",
  "alias": "Swift Falcon",
  "deviceType": "phone",
  "os": "android",
  "appVersion": "1.0.0",
  "protocolVersion": 1,
  "capabilities": ["chunked", "parallel", "sha256", "web-share", "remote-download"],
  "maxParallelStreams": 4
}
```

#### 4.4.3 Offer request and response

```json
// Request
{
  "sender": { "deviceId": "...", "alias": "Ephraim's Phone", "deviceType": "phone", "fingerprint": "..." },
  "files": [
    { "fid": "f1", "name": "lecture.mp4", "size": 734003200, "mime": "video/mp4", "sha256": "...", "modified": 1731700000 },
    { "fid": "f2", "name": "notes.pdf", "size": 2411520, "mime": "application/pdf" }
  ],
  "totalSize": 736414720
}

// Response when accepted
{
  "sid": "b92f...",
  "accepted": true,
  "files": {
    "f1": { "token": "t_...", "resumeFrom": 0 },
    "f2": { "token": "t_...", "resumeFrom": 0 }
  },
  "chunkSize": 4194304,
  "streams": 4
}

// Response when declined
{ "accepted": false, "reason": "declined" }
```

The offer request is held open until the receiver decides or a 60-second timeout elapses (reason `timeout`).

#### 4.4.5 File upload

`PUT /api/v1/session/{sid}/file/{fid}?token=...` with headers:

- `Content-Range: bytes <start>-<end>/<total>`
- `Content-Type: application/octet-stream`

The receiver writes into a temporary part file at the given offset and replies `204`. Up to `streams` concurrent requests may target different ranges of the same file or different files.

#### 4.4.6 Completion

After all ranges succeed, the sender calls `complete` with the SHA-256 of the file. The receiver hashes the part file (or verifies the rolling hash it computed), renames it to its final name, and replies `200` with `{ "verified": true }`. On mismatch it replies `422` and the sender retries the affected ranges.

#### 4.4.7 Resume

1. The sender reconnects and calls `GET .../status`.
2. The receiver returns the list of byte ranges it has durably written.
3. The sender uploads only the missing ranges.

Chunks are acknowledged only after being flushed to disk, so a crash on either side loses at most the in-flight chunks.

### 4.5 Session state machine

```
Created -> Offered -> (Declined | TimedOut | Accepted)
Accepted -> Transferring -> (Verifying -> Completed | Failed | Cancelled | Paused)
Paused -> Transferring | Cancelled
Failed -> Transferring (user retry or automatic resume)
```

Per-file states: `Pending, Sending, Verifying, Done, Failed, Skipped`.

### 4.6 Throughput design

- Chunk size defaults to 4 MiB and adapts: starts at 1 MiB, doubles while throughput improves, halves on retransmission.
- Parallel streams: start at 2, increase to the negotiated maximum when the link is fast and CPU headroom exists.
- Zero-copy reads from the source file where the platform allows (memory-mapped or direct file handles).
- Hash computation runs on a separate thread and overlaps with I/O.
- Small files (under 256 KiB) are batched into a single stream to avoid per-request overhead.
- Compression is not applied to already-compressed types; text and uncompressed formats may use a streaming compressor when the CPU is idle.

### 4.7 Link negotiation (fast paths)

After discovery, both devices exchange capabilities and choose the best available link:

| Rank | Link | Platforms | Notes |
|---|---|---|---|
| 1 | Direct peer-to-peer Wi-Fi | Android, Windows (and between Apple devices where the platform allows) | Fastest; needs no router |
| 2 | Hotspot hosted by one device | Android, Windows, Linux | The other device joins using credentials from a QR |
| 3 | Shared Wi-Fi or Ethernet | All | The baseline path |
| 4 | Web-share | Any browser | Receiver needs no install |

Rules:
- The protocol above runs unchanged on every link; only the address changes.
- If a faster link fails to establish within 8 seconds, the session continues on the current link.
- iOS cannot create or programmatically join networks; the app shows a guided "join this network" step with the QR and returns when the network is joined.
- Switching links mid-session is allowed. The session resumes using the status endpoint.

### 4.8 Web-share

- A device can start a temporary HTTPS server and show a QR and short URL such as `https://192.168.1.20:53320/s/k3x9`.
- Two modes: **Send** (the page lists files with download buttons or a zip-all) and **Receive** (the page has an upload box).
- Protection: random path segment, 6-digit PIN entered on the web page, automatic expiry (default 10 minutes), and single-session lock.
- The page is a single self-contained document with no external resources.
- Browsers will warn about the self-signed certificate; the page explains how to proceed and shows the fingerprint to compare.

### 4.9 Text and clipboard

- `POST /api/v1/text` carries up to 64 KiB of text.
- The receiver shows a card with Copy, Open (if a link), and Save as file.
- Clipboard send is manual, never automatic, to prevent silent clipboard leaks.

### 4.10 Edge cases

| Case | Behavior |
|---|---|
| Duplicate filename on the receiver | Append a counter: `name (1).ext` |
| Insufficient space | The receiver rejects the offer with `no-space` and the needed size |
| File changes during sending | The sender detects modified time or size change and restarts the file |
| Peer switches network mid-transfer | Both sides retry with exponential backoff for up to 2 minutes, then pause |
| App suspended on iOS | The session pauses; a local notification offers to return and resume |
| Folder with deep nesting or symlinks | Relative paths preserved up to depth 32; symlinks are skipped |
| Illegal filename characters on the target OS | Replace with `_` and record the original in session metadata |
| Very large single file (over 4 GiB) | Supported; ranges are 64-bit |
| Receiver offline when sender starts | Offer fails fast with `unreachable`; the item remains in the Send queue |

---

## 5. Download subsystem

### 5.1 Download job model

A job has: `id`, `url`, `kind` (direct, media, stream, torrent), `state`, `destination`, `filename`, `totalBytes`, `downloadedBytes`, `segments[]`, `headers`, `cookiesRef`, `createdAt`, `priority`, `retryCount`, `sourcePage`, `extractorResult`, `postProcess[]`.

### 5.2 Job state machine

```
Queued -> Probing -> Downloading -> (Merging -> Processing)? -> Completed
Downloading <-> Paused
Any active state -> Failed (retryable or terminal)
Any state -> Cancelled
Waiting-for-network / Waiting-for-schedule (substates of Queued/Paused)
```

### 5.3 Direct download engine

**Probing**

1. Send `HEAD`; if rejected, send `GET` with `Range: bytes=0-0`.
2. Learn: total size, range support, `ETag`, `Last-Modified`, `Content-Disposition` filename, content type.
3. Resolve redirects and record the final URL; keep the original for resume.
4. Derive the filename: `Content-Disposition`, then URL path, then a generated name with an extension from the MIME type.

**Segmentation**

| File size | Segments |
|---|---|
| Under 1 MiB | 1 |
| 1 to 20 MiB | 2 to 4 |
| 20 MiB to 1 GiB | 4 to 8 |
| Over 1 GiB | 8 to 16 |

Segment planning is dynamic: when a segment finishes early, the largest remaining segment is split in half and the free connection takes the second half (work stealing). Never exceed the per-host connection cap (default 8) to avoid server bans.

**Writing**

- Preallocate a sparse file where the file system supports it, otherwise write part files and join on completion.
- Write progress records (segment id, start, end, bytes done) to SQLite every 512 KiB or 2 seconds, whichever comes first.

**Resume**

- On resume, validate `ETag` or `Last-Modified`. If it changed, ask the user whether to restart.
- Request only the missing byte ranges.
- If the server does not support ranges, resume is impossible: restart with a single connection and tell the user.

**Retry policy**

| Failure | Action |
|---|---|
| Connection reset or timeout | Retry with exponential backoff: 1, 2, 4, 8, 16, 30 seconds, max 10 attempts |
| HTTP 429 or 503 | Honor `Retry-After`; reduce segments by half |
| HTTP 403 | Retry once with refreshed headers; if media, re-resolve through the extractor |
| HTTP 404 or 410 | Terminal failure |
| Disk full | Pause, notify, resume when space is available |
| Checksum mismatch (if provided) | Re-download the affected segments once, then fail |

**Network awareness**

- Pause automatically on loss of connectivity; resume on return.
- Optional "Wi-Fi only" and "avoid metered networks" per job and globally.
- Switch between Wi-Fi and mobile data without losing progress.

### 5.4 Queue and scheduling

- Configurable concurrent jobs (default 3 on mobile, 5 on desktop).
- Priority: High, Normal, Low; drag to reorder.
- Schedules: start time, end time, days of the week, and an action at the end (stop, sleep, shut down on desktop).
- Speed limits: global, per job, and a separate limit during scheduled windows.
- Batteries: pause on low battery (configurable) on mobile.

### 5.5 Media extraction

**Extractor interface**

```
resolve(url, options) -> {
  title, uploader, duration, thumbnails[],
  formats: [
    { id, container, videoCodec, audioCodec, width, height, fps,
      bitrate, size, protocol (https|hls|dash), url, headers, isAudioOnly, isVideoOnly }
  ],
  subtitles: [{ lang, format, url }],
  isLive, drm: false
}
```

**Behavior**

- The extractor is a separate, versioned component made of **signed WebAssembly plugins**, one per site or site family, run by the backend's extractor host in a sandbox. The app checks for updates daily (when allowed) and applies them without an app release. A failed update rolls back automatically.
- Plugins have no direct access to the network, file system, or clock. They call host functions the backend provides: `fetch(request)` (the backend performs it, applying redirect limits, size limits, and the cookies the user allowed for that host), `log`, and `now`. This keeps untrusted site-specific code unable to exfiltrate data or touch files.
- Every plugin package is verified against a pinned Ed25519 key before it is loaded; unsigned or tampered packages are rejected.
- Each call has a CPU-time budget (fuel metering), a memory cap, and the 30-second wall-clock limit below.
- Each extraction runs with a time limit (30 seconds) and a memory cap.
- If the extractor reports DRM, the job is refused with a clear explanation.
- Results are cached for a short time (10 minutes) because signed media URLs expire.

**Format selection UI model**

- Presets: Best, Balanced (1080p or lower), Data saver (480p), Audio only (best), Custom.
- When video-only and audio-only formats are chosen, the job muxes them into one file.
- Show estimated size before starting.

### 5.6 In-app browser and sniffer

- Uses the platform web view with request interception.
- Detection rules: response content types (`video/*`, `audio/*`, `application/vnd.apple.mpegurl`, `application/dash+xml`), file extensions, and known manifest patterns.
- Detected items appear in a floating "download" badge with a count; tapping shows a sheet with names, sizes, and quality.
- De-duplicate by normalized URL; ignore tiny items (tracking pixels, thumbnails) with a size threshold of 50 KiB unless the type is audio or video.
- Pass the page's cookies and `Referer` to the download so authenticated media works.
- Private browsing mode: no history, cookies cleared on exit.
- Ad and tracker blocking is optional, using a local filter list; the browser never injects its own ads.
- On iOS the web view limits request visibility; detection relies on in-page script hooks and manifests, and some media will require the share-to-app or paste-link path instead.

### 5.7 Stream download (HLS and DASH)

1. Fetch the manifest; select a variant by the chosen quality.
2. Parse the fragment list; reject if the manifest declares encryption that requires a license server (DRM). Plain AES-128 segment encryption with an openly retrievable key is supported.
3. Download fragments in parallel (default 8 on desktop, 4 on mobile), each with retry.
4. Maintain a fragment index in SQLite so interrupted jobs resume at the fragment level.
5. Concatenate or remux fragments into the target container.
6. For live streams, record until stopped; show elapsed time and size; support a maximum duration limit.

### 5.8 Post-processing pipeline

Steps are configurable per job and run in order:

| Step | Description |
|---|---|
| Mux | Combine separate video and audio into MP4 or MKV without re-encoding |
| Audio extract | Save audio as MP3, AAC, Opus, or FLAC |
| Convert | Optional re-encode to a chosen preset (off by default; battery heavy) |
| Subtitles | Download and embed or save alongside as `.srt` |
| Metadata | Embed title, artist, thumbnail; write tags |
| Rename and move | Apply the filename template and destination rule |
| Verify | Play-test the first seconds or inspect container integrity |

Filename template tokens: `{title}`, `{uploader}`, `{date}`, `{resolution}`, `{ext}`, `{index}`.

### 5.9 Playlists and batches

- Detect playlist or multi-item pages; show a checklist with titles, durations, and a select-all.
- Each selected item becomes its own job in a group; the group shows combined progress.
- Options: numbering prefix, subfolder named after the playlist, skip items already in the library (by source URL).

### 5.10 Authenticated downloads

- Users can import cookies from the in-app browser session or paste headers for a specific host.
- Credentials are stored encrypted, scoped per host, and attached only to requests to that host.
- The user can clear them per host at any time.

### 5.11 Torrent module (optional, P2)

- Off by default; enabled in settings with an explanatory notice.
- Magnet and `.torrent` support, file selection, sequential download option.
- Seeding limits (ratio and time) with defaults that stop seeding after completion.
- Desktop first; mobile only with Wi-Fi-only and charging-only defaults.

### 5.12 Desktop browser extension

**Capabilities**
- Capture download links and offer "Download with DropGrab".
- Detect playing media and show a floating button with format choices.
- Context menu: Download link, Download media, Send to device, Add to queue.
- Intercept normal downloads above a configurable size and take them over (toggle).

**Communication**
- The extension talks to `dropgrab-nmhost`, a small Rust binary registered with the browser as its native-messaging host. The host relays messages to the backend daemon (`dropgrabd`) through the control API on the user-private local socket.
- Messages are JSON, versioned, and authenticated with a per-install secret so other local software cannot inject jobs. The host accepts messages only from the extension identifiers it was registered for.
- If the daemon is not running, the host starts it (with a visible notice) or the extension falls back to the browser's own download.
- The backend, not the extension, performs the actual download, so cookies and headers for the page are passed to it as part of the capture message and scoped to the target host.

**Message types**
`hello`, `capture-download`, `capture-media`, `send-to-device`, `queue-add`, `status`, `ping`.

---

## 6. Integration between download and transfer

### 6.1 Download then send

- Every completed download shows a **Send** button. Selecting it opens the nearby-device list with the file preselected.
- Optional rule: "After download, send to [device]" for a specific job or a category (such as videos to the TV box or a laptop).

### 6.2 Received items as library entries

- Received files are indexed exactly like downloaded files and can be re-sent, previewed, or deleted from the same screens.
- Provenance is shown: "Downloaded from example.com" or "Received from Swift Falcon".

### 6.3 Remote download queue

Use case: the phone has a slow connection; a paired computer has a fast one.

1. On the phone, the user picks "Download on [laptop]" for a link.
2. The phone calls `download-request` on the paired laptop (only allowed for Trusted devices with the setting enabled on the laptop).
3. The laptop resolves, downloads, and post-processes the item.
4. When done, the laptop offers a send session back to the phone, or the phone fetches it on demand.
5. Progress updates are pushed to the phone while both are reachable.

Safeguards: the laptop shows who requested what; a per-device allowance (max size and daily count) can be set; extraction results are never executed, only downloaded.

### 6.4 Share intake

- The app registers as a share target for URLs and files on mobile and desktop.
- A shared URL opens a sheet: Download, Download on [device], Send link to [device], Open in browser.
- A shared file opens: Send to device, Save to library.

---

## 7. Library and storage

### 7.1 Database schema (SQLite)

```sql
CREATE TABLE items (
  id            TEXT PRIMARY KEY,
  path          TEXT NOT NULL,
  display_name  TEXT NOT NULL,
  mime          TEXT,
  kind          TEXT,            -- video, audio, image, document, archive, app, other
  size          INTEGER,
  sha256        TEXT,
  origin        TEXT NOT NULL,   -- download | received | imported
  source_url    TEXT,
  source_device TEXT,            -- deviceId for received
  duration_ms   INTEGER,
  thumbnail     TEXT,
  created_at    INTEGER NOT NULL,
  last_opened   INTEGER,
  favorite      INTEGER DEFAULT 0
);

CREATE TABLE download_jobs (
  id TEXT PRIMARY KEY, url TEXT NOT NULL, kind TEXT, state TEXT,
  filename TEXT, destination TEXT, total_bytes INTEGER, done_bytes INTEGER,
  headers_json TEXT, cookies_ref TEXT, priority INTEGER, retry_count INTEGER,
  group_id TEXT, extractor_json TEXT, post_json TEXT,
  created_at INTEGER, updated_at INTEGER, error TEXT
);

CREATE TABLE segments (
  job_id TEXT, idx INTEGER, start INTEGER, end INTEGER, done INTEGER,
  PRIMARY KEY (job_id, idx)
);

CREATE TABLE fragments (
  job_id TEXT, idx INTEGER, url TEXT, done INTEGER, size INTEGER,
  PRIMARY KEY (job_id, idx)
);

CREATE TABLE devices (
  device_id TEXT PRIMARY KEY, alias TEXT, device_type TEXT, os TEXT,
  fingerprint TEXT, trust TEXT, first_seen INTEGER, last_seen INTEGER,
  allow_remote_download INTEGER DEFAULT 0, auto_accept INTEGER DEFAULT 0
);

CREATE TABLE sessions (
  sid TEXT PRIMARY KEY, direction TEXT, peer_id TEXT, state TEXT,
  total_bytes INTEGER, done_bytes INTEGER, started_at INTEGER, ended_at INTEGER, error TEXT
);

CREATE TABLE session_files (
  sid TEXT, fid TEXT, name TEXT, size INTEGER, sha256 TEXT, state TEXT,
  item_id TEXT, PRIMARY KEY (sid, fid)
);

CREATE TABLE rules (
  id TEXT PRIMARY KEY, match_json TEXT, action_json TEXT, enabled INTEGER
);
```

### 7.2 Folder layout

Default root: `DropGrab/` with subfolders `Videos`, `Music`, `Images`, `Documents`, `Archives`, `Apps`, `Received`, `Other`. Folder choice is by detected kind; users can override per category.

### 7.3 Platform storage behavior

| Platform | Approach |
|---|---|
| Android | Scoped storage; user picks a root through the system folder picker once; media can optionally be registered with the media index so it appears in gallery and music apps |
| iOS | App container plus the Files app; user can export to Photos (images and videos) or Files |
| Windows, macOS, Linux | User-selected folder; default under the user's Downloads or Documents |

### 7.4 Integrity and cleanup

- Part files live in a hidden temp folder and are removed on cancel or after 7 days of inactivity.
- On startup, reconcile the database with the file system: mark missing files, offer to remove stale entries.
- Duplicate detection by size plus hash; offer to merge or delete.

---

## 8. Platform specifics

### 8.1 Capability matrix

| Capability | Android | iOS | Windows | macOS | Linux |
|---|---|---|---|---|---|
| Same-network send/receive | Yes | Yes (foreground) | Yes | Yes | Yes |
| QR pairing | Yes | Yes | Yes | Yes | Yes |
| Bluetooth LE discovery | Yes | Foreground only | Yes | Yes | Varies |
| Direct peer-to-peer or hotspot link | Yes | Guided join only | Yes | Limited | Yes |
| Background receive | Foreground service | Limited | Tray | Menu bar | Tray |
| Direct downloads | Yes | Yes | Yes | Yes | Yes |
| In-app browser detection | Full | Limited | Via extension | Via extension | Via extension |
| Extractor | Via host or bundled | Via host | Bundled | Bundled | Bundled |
| Browser extension | n/a | n/a | Yes | Yes | Yes |
| Share intents | Yes | Yes | Context menu | Services menu | File manager integration |

### 8.2 Android notes

- Foreground service with a persistent notification (progress, pause, cancel actions) while a transfer or download is active.
- Request permissions on demand: notifications, nearby devices (radio discovery), camera (QR), local network.
- Battery optimization: guide the user to exempt the app only if background downloads are interrupted; never request exemption on first launch.
- Handle storage through scoped access; avoid broad file permissions.

### 8.3 iOS notes

- Declare the local-network usage reason and the service types used for discovery; the first send triggers the system prompt, so show a pre-prompt explaining why.
- Downloads use background sessions where possible so they continue when the app is suspended; transfers do not, so the app warns the user to keep it open and uses a "keep awake" option while a session is active.
- Share extension for links and files.
- The Rust backend runs in-process. Eligible direct downloads are delegated to the OS background transfer service through the `BackgroundTransfer` adapter so they continue when the app is suspended; the backend reconciles state on return. WebAssembly extractor plugins run in an interpreter-based runtime because just-in-time code generation is not permitted.
- Save-to-Photos for images and videos is a user action with a clear permission explanation.

### 8.4 Desktop notes

- The backend daemon (`dropgrabd`) with its tray or menu-bar icon starts at login (optional) and listens for transfers; the UI is a separate client that can be closed without stopping work.
- Drag-and-drop: drop files on a device tile to send; drop a URL on the window to download.
- Firewall: on first run, the installer or app explains and requests the rule needed for local network listening; if blocked, the UI shows a diagnostic with the fix.
- Auto-update with signed packages.

### 8.5 Web-share receiver compatibility

Works in current mobile and desktop browsers. Large uploads use chunked requests with resume; very old browsers fall back to a single request with a size warning.

---

## 9. User experience design

### 9.1 Information architecture

**Mobile (bottom tabs):** Nearby, Downloads, Browser, Library, Settings.
**Desktop (sidebar):** Nearby, Downloads, Browser (optional), Library, Devices, Settings.

### 9.2 Key screens

**Nearby (home)**
- Top: your device name, a visibility toggle (Everyone nearby / Trusted only / Hidden), and a QR button that shows your pairing code.
- Center: animated radar with discovered devices as tiles (icon by type, alias, and a trusted badge).
- Bottom: "Send" action row: Files, Folder, Photos, Text, Paste link.
- Drag and drop on desktop; tapping a device with files selected starts the offer.

**Incoming request sheet**
- Sender name and trust badge, file count, total size, list of file names with thumbnails.
- Buttons: Accept, Decline; options: Accept and always trust this device, Choose a different save location.

**Transfer progress**
- Per-session card with speed, ETA, and a collapsible file list.
- Actions: Pause, Cancel; after completion: Open, Show in library, Send to another device.

**Downloads**
- Paste or type a link at the top; a clipboard chip appears when a link is on the clipboard.
- Tabs: Active, Queued, Completed, Failed.
- Each card: thumbnail, title, source host, progress ring, speed, ETA, and quick actions (pause, send, delete).
- Tap a card for details: segments view, headers, retry history, log.

**Format picker (bottom sheet)**
- Header with thumbnail, title, and duration.
- Preset chips, then an expandable list of formats with resolution, codec, size, and a checkbox for audio-only.
- Destination and filename template at the bottom; primary button "Download".

**Browser**
- Address bar with search, site shortcuts, tabs, a floating detect badge.
- A bottom sheet lists detected items; long-press to copy link or download on another device.

**Library**
- Filter chips: All, Videos, Music, Images, Documents, Archives, Apps, Received.
- Grid or list toggle; multi-select toolbar: Send, Share, Move, Delete.
- Item detail: preview, provenance, size, hash, actions.

**Devices**
- List of trusted and recent devices; per-device switches for Auto-accept and Allow remote downloads; Block and Forget.

### 9.3 Core flows

**Flow A: send a file to a nearby device**
1. Open Nearby; tap Files and choose items.
2. Tap a device tile.
3. Receiver sees the request and accepts.
4. Progress appears on both sides; notification on completion.

**Flow B: first-time pairing without discovery**
1. Receiver opens the QR.
2. Sender taps Scan, scans, confirms the verification code on both screens.
3. Session begins; the receiver is offered "Trust this device".

**Flow C: download a video and send it to a laptop**
1. Paste a link or use the browser.
2. Pick a preset in the format sheet and start.
3. When finished, tap Send, choose the laptop, and the transfer starts.

**Flow D: download on a computer from the phone**
1. Share a link to DropGrab on the phone.
2. Choose "Download on Laptop".
3. The laptop downloads; the phone shows live progress and offers to pull the file when ready.

### 9.4 Empty, loading, and error states

| State | Presentation |
|---|---|
| No devices found | Explain likely causes (different networks, firewall, hidden mode) with buttons for Scan QR, Refresh, and Troubleshoot |
| Permission denied | Inline explanation and a button to open settings |
| Download failed | Reason in plain language, Retry, and View details |
| Unsupported link | Offer to download as a direct file or open in the browser |
| DRM-protected content | Clear message that this content cannot be saved and why |
| Out of space | Show needed vs available space and open the storage manager |

### 9.5 Visual and interaction principles

- Calm, high-contrast interface with a single accent color; light and dark themes follow the system.
- Large touch targets (48 dp minimum), one-handed reach for primary actions on phones.
- Motion is used to show status (radar pulse, progress), never decoration; respects reduced-motion settings.
- Consistent iconography for file types.

### 9.6 Accessibility

- Full screen-reader labeling, including progress announcements at 25% steps.
- Dynamic type and scalable layouts.
- Color is never the only status indicator.
- Keyboard navigation and shortcuts on desktop.

### 9.7 Localization

- All strings externalized; right-to-left layouts supported.
- Number, date, and size formatting follow the locale.
- Initial languages: English, French, Spanish, Arabic, Hausa, Twi, Swahili, Portuguese; more by community contribution.

---

## 10. Security and privacy

### 10.1 Threat model

| Threat | Mitigation |
|---|---|
| Eavesdropping on the local network | TLS 1.3 for every session |
| Impersonation of a device | Fingerprint pinning; verification code on first trust; warning on certificate change |
| Unsolicited files (spam, malware) | Approval required by default; visibility modes; rate limit per sender; block list |
| Malicious file contents | Files never auto-open or auto-install; warning for executables and installers; optional hash check |
| Path traversal in file names | Sanitize and normalize all received paths; reject `..`, absolute paths, and reserved names |
| Resource exhaustion by a peer | Caps on concurrent sessions, file count, and offer size; timeouts; per-peer quotas |
| Tracking of users via radio beacons | Rotating BLE tokens; no stable identifiers in broadcast |
| Local malware injecting download jobs | User-private local socket or pipe with peer-credential checks; extension messages require the install secret |
| Malicious web page driving the control API | Loopback TCP off by default; token required; `Origin` and `Host` checks; no CORS headers |
| Malicious or compromised extractor plugin | WebAssembly sandbox with no ambient authority; backend-mediated fetch; signature verification; CPU and memory limits |
| Memory-safety bugs in parsers | Rust by default; `unsafe` confined and audited; fuzz targets for every untrusted-input parser |
| Web-share abuse | One-time path, PIN, expiry, single session, local network only |
| Credential leakage | Cookies and headers encrypted at rest, scoped per host, cleared on request |
| Tampered extractor update | Signed update packages with pinned public key; rollback on failure |
| Rogue remote-download request | Only from Trusted devices that the user enabled; visible prompt or allowance; size and count caps |

### 10.2 Data handling

- No account, no server-side storage. The app contacts the internet only to: perform user-requested downloads, check for app and extractor updates, and (optionally) fetch filter lists.
- Telemetry is off by default. If enabled, it contains only anonymous counters (crashes, feature usage) with no file names, URLs, or device identifiers.
- Logs stay on the device, are redacted of URLs by default, and are exported only by the user.
- Settings has a "Clear everything" action that wipes the database, keys, and caches.

### 10.3 Permissions principle

Request each permission at the moment it is needed with a short explanation; the app works with reduced capability when a permission is declined (for example, the QR path still works without Bluetooth).

---

## 11. Performance targets

| Scenario | Target |
|---|---|
| Large file send on 5 GHz Wi-Fi | 70% or more of measured link throughput |
| Direct peer-to-peer link between two phones | 20 MB/s or more where the hardware supports it |
| 100 small files (total 50 MB) | Under 15 seconds on a typical home network |
| Discovery on same network | Peer visible within 2 seconds |
| Download acceleration vs single connection (range-capable server) | 2 to 5 times faster on capped per-connection servers |
| Memory use during a 10 GB transfer | Under 150 MB on mobile |
| Idle background footprint (desktop helper) | Under 60 MB and under 1% CPU |
| Battery impact during a 10-minute transfer | Comparable to video playback at similar screen state |

Optimization techniques: adaptive chunking and parallelism, zero-copy I/O, hash overlap on a separate thread, connection reuse, small-file batching, throttled progress updates (4 UI updates per second at most).

---

## 12. Background behavior and notifications

### 12.1 Background rules

| Platform | Behavior |
|---|---|
| Android | Foreground service during active work; stops when idle |
| iOS | Downloads continue via background sessions; transfers pause when suspended and resume on return; a notification invites the user back |
| Desktop | Helper process keeps working after the window closes |

### 12.2 Notification types

- Incoming transfer request (with Accept and Decline actions where supported).
- Transfer or download progress (ongoing, silent).
- Completed (tap to open or send).
- Failed (tap to retry).
- Scheduled download started or finished.
- Remote-download request received from a trusted device.

Notifications never contain file names if the user turns on "Hide details on lock screen".

---

## 13. Settings

| Group | Options |
|---|---|
| General | Device name, language, theme, start at login (desktop) |
| Nearby | Visibility mode, auto-accept for trusted devices, default save folder, ask before accepting, hotspot preferences |
| Downloads | Default folder, concurrent jobs, segments per file, speed limit, Wi-Fi only, metered networks, battery rules, retry limits |
| Media | Default preset, audio format, subtitle languages, filename template, post-processing steps |
| Browser | Search engine, tracker blocking, private mode default, clear data |
| Remote | Allow remote download requests, per-device allowances |
| Torrent (if enabled) | Ports, limits, seeding rules |
| Storage | Cleanup, duplicates, folder layout |
| Privacy | Telemetry, clear history, clear credentials, reset keys |
| Updates | App updates, extractor updates, release channel |
| Advanced | Network interface selection, multicast and port settings, diagnostics, export logs |

---

## 14. Error handling and diagnostics

### 14.1 Error taxonomy

Errors carry a stable code, a user-facing message, and a suggested action.

| Code | Meaning | User action offered |
|---|---|---|
| NET_UNREACHABLE | Peer cannot be reached | Check same network, try QR |
| TLS_MISMATCH | Fingerprint differs from expected | Re-pair the device |
| DECLINED | Receiver declined | None |
| TIMEOUT | No response in time | Retry |
| NO_SPACE | Not enough storage | Free space |
| SRC_CHANGED | Remote file changed on server | Restart or keep partial |
| RANGE_UNSUPPORTED | Server cannot resume | Restart with one connection |
| EXTRACT_FAILED | Extractor could not resolve | Update extractor, try direct link |
| DRM_PROTECTED | Content is protected | None |
| AUTH_REQUIRED | Login needed | Sign in through the browser tab |
| HASH_MISMATCH | Integrity check failed | Retry |
| PERMISSION | OS permission missing | Open settings |

### 14.2 Diagnostics screen

- Network summary: interface, IP, multicast reachable (test), port bound, firewall hint.
- Last 100 log lines, redacted, with copy and share.
- One-tap "Run connectivity test" with another device selected.

---

## 15. Testing strategy

| Level | Scope |
|---|---|
| Unit | Segment planner, retry logic, filename sanitizing, template engine, state machines, hash and range math |
| Property tests | Range merging and resume invariants (no gaps, no overlaps); path sanitization against hostile inputs |
| Integration | Two backend instances talking over loopback and over emulated networks with loss, latency, and reordering |
| Protocol conformance | A test suite that any implementation of the session protocol must pass |
| Device matrix | Current and two previous major versions of each OS; low-end, mid, and flagship phones; Windows, macOS, and common Linux desktops |
| Network scenarios | Home router, guest network with client isolation, corporate network with multicast blocked, hotspot, VPN on |
| Download servers | Range-supporting, range-less, throttling, redirecting, expiring-URL, slow-start servers (local test fixtures) |
| Media | A corpus of manifests and containers (multi-bitrate, separate audio, subtitles, discontinuities) |
| Soak | 24-hour queue with random network drops and forced process kills |
| Security | Fuzzing of the session API and manifest parsers; static analysis; dependency audit; threat-model review each release |
| Rust-specific | `cargo-fuzz` targets for every untrusted-input parser; Miri on the `unsafe` code; `loom` or `shuttle` model tests for channel and cancellation logic; property tests (`proptest`) for range math; benchmarks (`criterion`) with regression thresholds for hashing, chunk I/O, and the segment scheduler |
| Control API | Contract tests generated from the shared type schema; reconnect and `resync` tests; hostile-client tests (oversized messages, malformed JSON, wrong `Origin`) |
| FFI | Smoke tests on Android and iOS emulators; panic-injection tests confirming that a Rust panic becomes an error code, not a crash; lifecycle tests (start, stop, kill, restart) |
| Cross-compilation | Every release target built and run through a minimal self-test in CI |
| Usability | Task-based tests: first send, first download, pair without discovery, remote download |
| Accessibility | Screen reader and keyboard-only passes on each platform |

Release gates: no open critical bugs, crash-free rate at target in the beta, resume tests passing, security checklist signed off.

---

## 16. Legal, policy, and distribution

This section is design guidance, not legal advice. Have counsel review it for the markets you launch in.

### 16.1 Content and copyright

- The app is a general-purpose download manager and file-transfer tool. The user decides what to download and is responsible for having the right to do so.
- First-run and settings pages show a plain-language notice about respecting copyright and site terms.
- The app refuses to process DRM-protected streams.
- No built-in catalog or search of third-party content is provided by the app itself.

### 16.2 Site terms and takedowns

- Some sites prohibit downloading in their terms. Keep the extraction feature separable (an updatable component) so it can be disabled or removed per distribution channel or region without affecting transfer and direct downloads.
- Provide a contact channel for rights holders and a process to respond to valid requests that concern the extractor component.

### 16.3 Distribution strategy

| Channel | Build contents |
|---|---|
| Mobile app stores | Transfer, direct downloads, library, browser without site-specific extraction, remote download hosting. Extraction features are omitted or limited as store rules require |
| Direct distribution (own website, alternative app repositories) | Full build including the extractor component |
| Desktop (own site and package managers) | Full build including the extractor and the browser extension |
| Browser extension stores | Capture and send features; no extraction logic inside the extension itself |

Maintain clear feature flags per channel so the same codebase produces each build.

### 16.4 Licensing of dependencies

- Maintain a software bill of materials; check each dependency's license for compatibility with the chosen product license.
- Muxing tools may carry copyleft obligations depending on how they are built and linked; prefer dynamic linking or separate processes and build only the needed components.
- Provide an in-app "Open source notices" page.

### 16.5 Privacy and regional compliance

- Because the app collects no personal data by default and has no accounts, obligations are minimal, but document the data flows (updates, optional telemetry) in a privacy policy.
- If optional telemetry or crash reporting is offered, obtain explicit consent and honor regional requirements.

### 16.6 Monetization options (compatible with the no-ads goal)

- Free core with optional one-time purchase for advanced features (scheduler rules, batch automation, torrent module, priority extractor updates).
- Optional paid desktop license for power users.
- No advertising SDKs and no sale of data.

---

## 17. Project structure and roadmap

### 17.1 Repository layout

```
dropgrab/
  Cargo.toml                # Rust workspace
  crates/
    dg-types/               # Shared types: commands, events, error codes, IDs (source of generated TS types)
    dg-db/                  # SQLite schema, migrations, writer task, queries
    dg-crypto/              # Certificates, fingerprints, trust store, signature verification
    dg-discovery/           # Multicast, subnet scan, QR pairing, BLE bridge
    dg-transport/           # Peer HTTPS server and client, chunking, resume, link negotiation
    dg-session/             # SessionService and the session state machine
    dg-segments/            # Segment engine, retry policy, range math
    dg-streams/             # HLS and DASH handling, fragment scheduler
    dg-extractor/           # Plugin host, host functions, signature checks, update client
    dg-media/               # Muxer trait and implementations
    dg-webshare/            # Temporary web-share server and page
    dg-library/             # Item index, previews, duplicates, rules
    dg-services/            # Remaining services (Device, Download, Scheduler, Settings, Notification, Update)
    dg-api/                 # Control API server, event bus, schema
    dg-platform/            # PlatformAdapter traits and desktop implementations
    dg-ffi/                 # uniffi bindings for Android and iOS
    dg-daemon/              # dropgrabd binary (desktop and headless)
    dg-cli/                 # dropgrab command-line client
    dg-nmhost/              # Native-messaging host for the browser extension
    dg-testkit/             # Fakes, fixtures, network emulation, test servers
  plugins/                  # Extractor plugins (WebAssembly) and signing tools
  clients/
    mobile/                 # React Native UI plus Kotlin and Swift adapter code
    desktop/                # Tauri UI
    web/                    # Headless web UI (shares components with desktop)
    extension/              # Browser extension
  protocol/                 # Peer protocol specification and conformance tests
  design/                   # Design system, assets, copy
  docs/
```

Dependency rule: `dg-types` depends on nothing internal; engine crates depend only on `dg-types`, `dg-db`, `dg-crypto`, and `dg-platform` traits; services depend on engine crates; `dg-api` depends on services; binaries and `dg-ffi` depend on `dg-api`. Clients depend on the generated schema only.

### 17.2 Team

| Role | Count |
|---|---|
| Rust backend engineer (transport, discovery, crypto, daemon) | 2 |
| Rust backend engineer (download engine, streams, extractor host, FFI) | 1 |
| Mobile engineer (React Native UI plus Kotlin and Swift platform adapters) | 2 |
| Desktop, web UI, and extension engineer | 1 |
| Designer (product and interaction) | 1 |
| QA / test automation | 1 |
| Product and project lead | 1 |

### 17.3 Phases

**Phase 0: Foundations (3 weeks)**
- Protocol specification and conformance tests
- Backend skeleton: workspace, service supervisor, event bus, bounded-channel conventions
- Control API with generated TypeScript types, local socket transport, and the FFI bridge (a "hello world" command and event on Android, iOS, and desktop)
- Identity, TLS with fingerprint pinning, database and migrations
- Platform adapter traits with desktop implementations and mobile stubs
- CI building all target triples
- Design system and clickable prototype

**Phase 1: MVP (10 weeks)**
- Discovery (multicast, scan, QR), session protocol, chunked resume
- Send and receive on Android, iOS, Windows, and macOS
- Direct download engine, queue, scheduler, clipboard detection
- Unified library with previews, share intents
- Beta with 100 users

**Phase 2: Media (8 weeks)**
- Extractor host and updater, format picker
- In-app browser with detection (Android first, then iOS limited mode)
- Desktop extension with native host
- Stream download and post-processing
- Web-share

**Phase 3: Speed and reach (8 weeks)**
- Direct link and hotspot negotiation, Bluetooth discovery
- Trusted devices, remote download queue
- Playlists, batch jobs, download rules
- Linux builds, localization expansion

**Phase 4: Extras (ongoing)**
- Torrent module, folder push, device migration
- Performance tuning, accessibility audits, community translations

### 17.4 Milestone exit criteria

| Milestone | Exit criteria |
|---|---|
| M1 (end of Phase 1) | Cross-OS sends succeed in all pairings; kill-and-resume tests pass; direct downloads accelerated and resumable |
| M2 (end of Phase 2) | Format picker works on the supported site set; extractor update and rollback verified; extension capture works in two desktop browsers |
| M3 (end of Phase 3) | Direct link negotiation falls back cleanly; remote queue works across NATs on the same LAN; localization in 8 languages |

---

## 18. Risks and open questions

### 18.1 Risks

| Risk | Impact | Likelihood | Mitigation |
|---|---|---|---|
| Store policies restrict download features | Distribution limits | High | Channel-specific builds; transfer-first store version |
| Extractor breakage as sites change | Core media feature degrades | High | Independently updatable extractor; fast release process; community fixes |
| iOS background restrictions | Transfer reliability | Medium | Foreground-first UX, resume, clear guidance |
| Multicast blocked on many networks | Discovery fails | High | QR and scan fallbacks; diagnostics screen |
| Peer-to-peer link support varies across devices | Inconsistent speed | Medium | Always fall back to shared-network path; test matrix |
| Mobile extraction cost (CPU, battery, size) | Poor experience | Medium | Remote host option; lightweight resolvers; lazy download of the component |
| Large-file performance on low-end phones | Slow or failed transfers | Medium | Adaptive chunking, memory caps, soak tests |
| Security issues in parsing untrusted manifests and files | Vulnerabilities | Medium | Fuzzing, sandboxed parsing, fast patch pipeline |
| Scope creep | Delays | High | Phase gating; P0 before P1 |
| Rust cross-compilation and FFI complexity on mobile | Slow builds, packaging issues | Medium | Prove the FFI pipeline in Phase 0; pin toolchains; CI for every target |
| WebAssembly runtime limits on iOS (no just-in-time code) | Slower extraction on iOS | Medium | Interpreter-based runtime; keep plugins small; prefer the remote host for heavy extraction |
| Mobile binary size growth | Larger downloads | Medium | Size-optimized profiles, feature flags per channel, strip and measure on every change |
| Rust hiring and learning curve | Schedule | Medium | Keep the Rust surface to the backend; clear crate boundaries; thorough contributor guide |
| Platform adapter drift between Android, iOS, and desktop | Inconsistent behavior | Medium | Shared adapter conformance tests run against each implementation |

### 18.2 Open questions

1. Is the product aimed at a public release or a personal/educational build? This changes distribution and legal posture.
2. Which regions come first? Network habits (router-less hotspot sharing vs home Wi-Fi) affect which link types to prioritize.
3. Should the paired computer be offered as the default download host for phones?
4. Which site categories must the extractor support at launch?
5. What is the minimum supported OS version per platform?
6. Will there be a paid tier, and which features sit behind it?

---

## Appendix A: Glossary

| Term | Meaning |
|---|---|
| Session | One transfer between two devices, containing one or more files |
| Segment | A byte range of a file downloaded over one connection |
| Fragment | One piece of a streamed video or audio |
| Fingerprint | SHA-256 of a device's certificate, used to identify it |
| Trusted device | A device the user explicitly verified and saved |
| Extractor | The updatable set of signed sandboxed plugins that turn a page link into downloadable formats |
| Backend (engine) | The Rust program or library that holds all product logic; UIs are clients of it |
| Control API | The command and event interface between a UI and the backend |
| Platform adapter | A per-OS implementation of a backend trait (files, radio, notifications, power, background transfer, share) |
| Daemon | `dropgrabd`, the desktop or headless backend process |
| Muxing | Combining video and audio streams into one file without re-encoding |
| Web-share | A temporary local web page that lets any browser send or receive files |

## Appendix B: Default values summary

| Parameter | Default |
|---|---|
| Discovery group and port | 239.255.42.99, UDP 53318 |
| Session port | TCP 53318 (fallback: next free port up to 53328) |
| Peer stale / removal | 30 s / 120 s |
| Offer timeout | 60 s |
| Chunk size | 1 MiB adaptive to 4 MiB |
| Parallel transfer streams | 2 adaptive to 4 |
| Max segments per download | 16 (8 per host by default) |
| Concurrent downloads | 3 mobile, 5 desktop |
| Retry attempts | 10, exponential backoff to 30 s |
| Web-share expiry | 10 minutes |
| QR pairing PIN validity | 2 minutes |
| Extractor update check | Daily |
| Extraction time limit | 30 s |
| Control API (desktop) | Local socket or named pipe; optional loopback TCP port 53319, off by default |
| Progress event rate | 4 per second per item |
| Event replay window | Last 1,000 events, then `resync` |
| Service channel capacity | 1,024 messages (bounded) |
| Service restart backoff | 1 s, 2 s, 4 s, up to 30 s |
| Database checkpoint | Every 512 KiB or 2 s |
| Mobile backend library size budget | 12 MB or less stripped, per architecture |
