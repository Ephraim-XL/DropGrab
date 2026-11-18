# DropGrab backend

Rust workspace implementing the DropGrab engine from `../dropgrab-app-design.md`:
control API (section 3.5), discovery (4.2), peer transfer over HTTPS (4), direct
downloads (5), pairing (4.3), web share (4.8), remote download requests (6.3) and
the library (7).

```
crates/
  dg-types   command/event names, error codes, peer wire types (no internal deps)
  dg-core    engine: api, discovery, peer, transfer, downloads, library, web share
  dg-daemon  dropgrabd binary: engine + loopback control API
```

## Build and test

Requires stable Rust (1.75+; developed on 1.100).

```sh
cd backend
cargo build --release          # binary at target/release/dropgrabd
cargo test --workspace         # unit + integration tests
```

The integration tests in `crates/dg-core/tests/` are real end-to-end runs, not mocks:

| file | what it does |
|---|---|
| `control_api_contract.rs` | every §3.5 command and event name exists, every command is reachable through `invoke`, unknown methods are answered with `UNKNOWN_METHOD`, and `/rpc` + `/events` work over a real loopback socket (token, Host check, SSE with seq). |
| `transfer_e2e.rs` | two engines talk over the peer HTTPS listener: offer → accept → 5 PUT ranges → complete → SHA-256 check → bytes verified byte-for-byte, library indexed; plus a pinned-fingerprint mismatch being refused. |
| `download_e2e.rs` | segmented direct download against a local HTTP server with `Range`: probe, download, resume, byte verification, library indexing; 404 → `NOT_FOUND`, bad URL → `INVALID_PARAMS`, pause/resume/cancel. |
| `mtls_e2e.rs` | mutual TLS: the claimed sender fingerprint must equal the client certificate presented on the handshake. Matching cert accepted (auto-accept path); mismatched cert refused with no `session.offered` event; absent cert refused; spoofed `text` sender refused with no `text.received` event; a non-participant cannot cancel another peer's session. Each refusal is paired with a positive control. |
| `webshare_e2e.rs` | real HTTPS share with the pinned certificate: PIN gate (page/file list hidden until unlock), 5 wrong PINs then 429 lockout, streamed download byte-for-byte, 409 single-session lock, unknown token → 404; an old share's expiry timer cannot stop a newer share; a stopped share stops serving. |
| `extractor_e2e.rs` | a page larger than the 2 MiB HTML cap whose cap offset falls inside a multi-byte character resolves without panicking (capped read, char-boundary truncation); plus unit tests for the truncation and the read cap in `extractor.rs`. |
| `downloads_slot.rs` | with `downloadsAtOnce = 1`: a probe that panics fails the job as `INTERNAL` and frees its concurrency slot, so a later download still starts and finishes; `downloads.cancel` on a job stuck in `probing` (server that never answers) removes it and frees the slot. |
| `pairing_e2e.rs` | pairing PIN attempt limit per pairing session: 5 wrong PINs → 429 and the correct PIN is refused while locked; a new `pair.qr_create` starts a fresh counter; 4 wrong PINs still accept the correct one. |
| `files_stage.rs` | `files.stage` round trip: a 200 KiB base64 file is staged under `<data-dir>/staging/<id>/`, sent with `transfer.start` by `stagedId` and by `path`, the bytes arrive intact, and the staged copy is removed when the session ends. Also: traversal names (`../../evil.txt`, `a/b.txt`, `..`, `/etc/passwd`) → `INVALID_PARAMS` with nothing written outside the staging folder; oversize → `PAYLOAD_TOO_LARGE`; the staging TTL sweep; `/rpc` accepts a >64 KiB body for `files.stage` only. |
| `origin_policy.rs` | `--allow-origin` policy: a configured origin with a valid token is accepted and gets `Access-Control-Allow-Origin`; an unlisted origin (including another `localhost` port) is 403 even with a valid token; no `Origin` keeps working with just the token; a bad or missing token is 401 for an allowed origin; the default (no flag) still rejects every browser origin; CORS preflight follows the same allowlist. |

## Run

```sh
# normal desktop run (control API on 127.0.0.1:53319, peer HTTPS on 53318)
./target/release/dropgrabd --data-dir /tmp/dg-data --downloads-dir /tmp/dg-downloads

# print the control-API token instead of only writing it to <data-dir>/api.token
./target/release/dropgrabd --print-token --no-discovery ...

# let the web UI served on http://localhost:8080 call the control API (repeatable)
./target/release/dropgrabd --allow-origin http://localhost:8080 --allow-origin http://127.0.0.1:8080
```

Flags: `--data-dir`, `--downloads-dir`, `--control-port` (0 = ephemeral), `--peer-port`
(0 = ephemeral), `--device-name`, `--no-discovery`, `--print-token`, `--allow-origin <origin>`
(repeatable; `--allow-origin=<origin>` also works).
Environment: `DROPGRAB_DATA_DIR`, `DROPGRAB_ADVERTISE_IP`.

The peer port is HTTPS with a self-signed certificate generated on first run
(`<data-dir>/identity/`); callers pin the full SHA-256 of the certificate. The control
API is HTTP on loopback only.

## Control API

* `POST /rpc` — `{ "id", "method", "params" }` → `{ "id", "result" | "error" }`
* `GET /events?after=<seq>` — server-sent events, `{ "seq", "type", "data" }`, `id:` = seq.
  A client that missed more than the replay window (1,000 events) gets a `resync` event and
  must reload through `downloads.list` / `session.list` / `library.query` / `devices.list`.
* Every request needs the bearer token: `Authorization: Bearer <token>` (token file `api.token`, mode 0600).
* `Host` must be `127.0.0.1` or `localhost`. An `Origin` header, if present, must be on the
  allowlist, which is empty (deny every browser origin) unless `--allow-origin` was passed.
  Requests without an `Origin` header are unaffected. For an allowed origin the response carries
  `Access-Control-Allow-Origin` (and `Vary: Origin`), and `OPTIONS /rpc` / `OPTIONS /events`
  answer the CORS preflight with 204; a preflight sends no token, so only `Host` and `Origin` are
  checked there. No CORS headers are ever sent to a non-allowed origin.
* Body cap 64 KiB for every method except `files.stage`, whose base64 payload may carry up to
  8 MiB (`staging::MAX_STAGE_BYTES`); a larger body from another method is answered with 413.
  100 commands/second.

```sh
TOKEN=$(cat /tmp/dg-data/api.token)
curl -s -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
     -d '{"id":1,"method":"backend.info"}' http://127.0.0.1:53319/rpc
```

All 41 commands from §3.5 are implemented (`dg_types::COMMANDS`); three additive extensions are
also accepted: `session.list` (re-read active sessions after a resync), `files.stage` and
`transfer.start` (browser uploads, below).

### Sending files from a browser

A browser file picker never yields a filesystem path, so the web client posts the bytes and gets
a daemon path back.

`files.stage` — request:

```json
{
  "id": 1,
  "method": "files.stage",
  "params": {
    "name": "photo.jpg",
    "content": "<base64 of the file>",
    "mime": "image/jpeg"
  }
}
```

`name` is required and must be a file name, not a path: `/`, `\`, `.` and `..` are rejected with
`INVALID_PARAMS`, and the accepted name is sanitized (illegal characters → `_`, 200 characters
max). `mime` is optional and currently only informational. `content` is standard base64 with
padding.

Response:

```json
{
  "id": 1,
  "result": {
    "id": "stg_9f1c4a0d2b7e5c38a1f0d6e4b92c7a51",
    "name": "photo.jpg",
    "size": 184320,
    "path": "/home/kofi/.local/share/dropgrab/staging/stg_9f1c.../photo.jpg"
  }
}
```

Errors: `INVALID_PARAMS` for a missing name/content, a name with a path separator, `..`/`.`,
an over-long name, or invalid base64; `PAYLOAD_TOO_LARGE` when the decoded file exceeds
`MAX_STAGE_BYTES` (8 MiB, the same cap as one peer upload chunk), with the message
`file is larger than the 8 MiB staging limit`.

Storage is `<data-dir>/staging/<id>/<name>`. The file is removed when a transfer that used it
ends (done, cancelled or failed), or after `STAGE_TTL` (1 hour) if never sent; the daemon also
sweeps leftovers at startup.

Send it with either the id or the path. `transfer.start` is an alias for `session.send` that
takes the same parameters:

```json
{ "id": 2, "method": "transfer.start",
  "params": { "peer": "<deviceId>", "items": [ { "stagedId": "stg_9f1c..." } ] } }
```

```json
{ "id": 3, "method": "session.send",
  "params": { "peer": "<deviceId>", "items": [ { "path": "/home/kofi/.local/share/dropgrab/staging/stg_9f1c.../photo.jpg" } ] } }
```

Response (both): `{ "sid": "<uuid>", "state": "offered" }`. An unknown `stagedId` is
`NOT_FOUND`. Mixing staged items with library ids / plain paths in one `items` array works.

## Behaviour notes and deviations

Kept from the design unless listed here (also recorded in `../PROGRESS.md`):

* **Control transport** is loopback TCP with a bearer token, not a Unix socket. The design
  allows this as the "Desktop (optional)" mode but the default-mode socket is not built.
* **Browser origins** are denied by default. `--allow-origin <origin>` (repeatable) adds an
  exact origin to the allowlist; the matching CORS headers and preflight are then served for it.
  There is no settings key for this: the policy belongs to the process launch, so a compromised
  token cannot widen it through `settings.set`.
* **Extractor host**: no signed WASM plugin sandbox exists in this build. `extractor.resolve`
  recognises direct audio/video URLs and returns one format; HTML pages without a matching
  site return `EXTRACT_FAILED`. `extractor.update` / `extractor.sites` report an empty
  plugin list.
* **HLS/DASH** manifests are not downloaded (`NOT_IMPLEMENTED`).
* **SQLite**: one connection behind a mutex (schema per §7.1); no writer-task/read-pool split.
* **Download segments** are planned once and not rebalanced; per-host cap 8.
* **Peer identity is mutual TLS.** The peer listener requests a client certificate on every
  connection (optional, so `info` and pairing work before trust exists). Every peer-protocol
  client presents this install's certificate. Handlers that trust a claimed `sender.fingerprint`
  (`announce`, `session/offer`, `text`, `download-request`, `pair`) return 403 unless the
  certificate on that connection has exactly that SHA-256. Session routes (`file/*`, `status`,
  `complete`, `cancel`) require the certificate of the session's own peer. Client certificates
  are accepted on proof of possession (the handshake signature), not checked against a CA,
  consistent with the pinned-certificate model. The receiver also refuses an offer whose
  fingerprint differs from a stored one for that device id.
* **Web share and the control API** do not use mutual TLS. Web share is a browser-facing page
  gated by its PIN. The control API is loopback with a bearer token.
* **Notifications** are `notification.request` events; no OS notification adapter.
* Not implemented: native messaging host (`dropgrab-nmhost`), muxer/transcoding, Tauri UI,
  autostart installers, headless-server web UI.

## Layout of the engine

| module | responsibility |
|---|---|
| `api.rs` | JSON-RPC dispatch, HTTP/SSE server, token, Host/Origin checks, rate limit |
| `events.rs` | monotonic seq, 1,000-event replay ring, broadcast bus |
| `discovery.rs` | multicast announce/listen, peer table, QR create/scan, pairing |
| `peer.rs` | peer HTTPS router and mTLS acceptor (records the presented cert per connection): info, offer, PUT range, status, complete, cancel, text, pair, download-request |
| `transfer.rs` | session state machine, sender driver, receiver parts, resume ranges |
| `downloads.rs` | probe, queue, segment planner, Range workers, pause/resume/cancel |
| `library.rs`, `db.rs` | library queries + SQLite schema (§7.1) |
| `webshare.rs` | temporary HTTPS share page with 6-digit PIN and expiry |
| `staging.rs` | `files.stage` browser uploads: 8 MiB cap, name validation (no path separators or `..`), `<data-dir>/staging/<id>/<name>`, cleanup on transfer end and by 1-hour TTL |
| `identity.rs` | certificate, fingerprint, pinned TLS clients, client-cert verifier, claimed-fingerprint check, verification code |
| `settings.rs` | typed settings, persisted in SQLite |
