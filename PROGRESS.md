# DropGrab Frontend Progress

**Rules**
1. Each agent records its task, todo, completed items, and progress in this file.
2. Only one agent works at a time.

**Sources:** `dropgrab-ui-design.md` (tokens, screens, components), `dropgrab-app-design.md` (architecture, control API, commands/events).

---

## Phase 1: Frontend build
**Owner:** Frontend agent
**Status:** COMPLETE

### Task
Build the DropGrab frontend (web client) from the two design docs.

### Todo
- [x] Set up project in `clients/web/` (static HTML/CSS/JS, no external network dependencies)
- [x] Implement design tokens (light + dark themes, follows system, manual override)
- [x] Implement screens: First run, Nearby (hero), Choose files, Incoming request, Transfer, QR pairing, Downloads, Choose quality, Browser, Library, Devices, Settings, Web share page
- [x] Mock data layer matching the control API command/event names
- [x] Verify in a real browser (screenshots, no console errors)
- [x] Write a short README with how to run

### Completed
- `clients/web/index.html` — app shell, pre-paint theme script, `file://` notice.
- `clients/web/css/tokens.css` — every colour, spacing, radius, type and motion token from the
  design doc's YAML front matter; dark values; 100/125/150% text-size scale.
- `clients/web/css/app.css` — app shell, nav (compact bar 64 / rail 80 / sidebar 240, content
  max 1120), top bar, buttons, device tiles, list rows, chips, inputs, switch/checkbox/radio,
  progress, banner, toast, sheets/dialogs, QR panel, settings groups, split desktop layout.
- `clients/web/js/icons.js` — 45 Lucide icons inlined from lucide-static 1.74.0 (ISC), 2 px
  stroke, `currentColor`; no CDN.
- `clients/web/js/mock/backend.js` — mock control API with the command names and event names
  from `dropgrab-app-design.md` §3.5 (`devices.*`, `session.*`, `downloads.*`, `extractor.resolve`,
  `library.*`, `remote.*`, `webshare.*`, `settings.*`, `pair.*`), timers for live progress,
  `localStorage` settings.
- `clients/web/js/mock/data.js` — sample content: Kofi's Pixel 8, five devices, the seven files,
  downloads (2 downloading, 1 paused, 1 finished), library, recent, quality presets, detected items.
- Screens: first run, Nearby (hero devices, visibility sheet, QR entry, link field, recent),
  Choose what to send sheet, Incoming request sheet, Transfer (offered/transferring/paused/done/
  cancelled, per-file states), QR pairing (code, fingerprint, scan → 4-digit check → trust),
  Downloads (banner, link intake, active/paused/finished, overflow menu), Choose a quality
  (chips on compact, radio rows on wider), Browser (address bar, page panel, "Download · 2 found",
  detected-items sheet), Library (search, filters, selection bar, delete dialog, empty/no-results),
  Devices (trusted cards, allowances, detail sheet, forget confirm), Settings (all eight groups,
  theme + text size controls), Web share receiver (`share.html`, code `504193`).
- Responsive layouts at 390 px, 780 px and 1440 px; left-aligned text throughout; one accent;
  focus rings, sheet focus trap, Escape handling, focus restore; reduced-motion respected.
- Keyboard: Ctrl/Cmd+1…5, Ctrl/Cmd+O, Escape; drag-and-drop files onto tiles, links anywhere.
- `clients/web/README.md` — how to run, routes, mock API, review hooks, known gaps.

### Progress log
- Created PROGRESS.md.
- Read both design documents. Confirmed no Lucide package is installed locally, so the icon
  paths were downloaded once from `lucide-static@1.74.0` and inlined into `js/icons.js`;
  the running app makes no network requests.
- Built tokens, shell and navigation, then screens in flow order. Sheet/dialog manager with
  focus trap and restore; toast with a 4 s timeout.
- Mock backend: `invoke(method, params)` + `subscribe(fn)` event bus with sequence numbers;
  200 ms tick advances active downloads and sessions; completion adds a library item and a
  notification event.
- Browser verification (Chromium via CDP, cache disabled, `python3 -m http.server`):
  - all ten routed screens plus `share.html` render, no horizontal overflow at 390 px, and no
    console errors or unhandled rejections on the swept routes;
  - flows exercised: choose files → send 3 files → tap device → transfer → done; incoming
    request → accept → transfer (662 MB) with per-file progress; pause/resume; direct link
    download; media link → quality picker → download; download pause/resume; library multi-select
    → delete dialog → delete; QR scan → 4-digit check → trust; web-share code entry and file list;
    theme System/Light/Dark with emulated `prefers-color-scheme`; Largest text size (24 px root)
    with no horizontal overflow; nearby-empty state.
  - screenshots checked at 1440×900 (dark) and 390×844 (light and dark); two are kept in
    `clients/web/screenshots/` (desktop Nearby dark, compact Downloads light).
  - fresh-visitor path checked: no storage → first run (no nav) → "Find nearby devices" →
    Nearby, and the completion flag survives a reload.
  - token audit: all 24 colour tokens and all seven spacing tokens in `css/tokens.css` match the
    design doc's YAML values exactly (dark values mapped onto the semantic names inside
    `:root[data-theme="dark"]`); no placeholder copy, exclamation marks or external URLs found
    in the client.
- Fixes made during verification: inlined icons had a nested SVG (license-comment regex), nav
  wordmark collided with the rail, first-run grid squeezed the content column, Save-on-event
  rendering used `requestAnimationFrame` (blocked in a headless tab) and now uses a timer,
  quiet-button rows had no flex gap, device rows' trailing controls stacked, compact `.col`
  had no vertical gaps, `fmtSize` rounded 48.6 MB to 49 MB, and download-link fields stacked
  the button instead of placing it beside the input. Also tightened off-scale values: select
  caret geometry, switch knob inset (4 px) and icon alignment margins now sit on the 4/8 scale.

### Re-verification (frontend agent) — 2026-11-18
Task: finish the two verification items left open after the Phase 1 build — a 390 px overflow
sweep across every documented route, and a token audit of `css/tokens.css` against the design
doc's YAML front matter — fix only real defects, and leave the running setup clean.

- [x] 390 px (390×844) overflow sweep: `#/first-run`, `#/nearby`, `#/downloads`, `#/quality`,
      `#/browser`, `#/library`, `#/devices`, `#/settings`, `#/qr`, `#/transfer` and `share.html`
- [x] Same sweep with the open sheets (Choose what to send, Incoming request) and with the
      Largest text setting (150%)
- [x] Token audit: all 24 colour tokens (12 light + 12 dark) and all 7 spacing tokens vs. the
      YAML values
- [x] Fresh-load console check on every route and `share.html`
- [x] Fix defects found (none were found; no files changed)

Progress log:
- Served `clients/web` on port 8000 (nothing was listening beforehand, so the server was
  started for this run and stopped again afterwards).
- Overflow measured as `documentElement.scrollWidth` vs `clientWidth` plus a scan for any
  element whose bounding box crossed the viewport edges. Result: scrollWidth == clientWidth
  on every route and on `share.html` (390 or 375 with the vertical scrollbar), zero offending
  elements. The same held with the "Choose what to send" and incoming-request sheets open, and with
  `data-text="largest"` on Nearby, Downloads, Library, Settings, Quality and Transfer.
- Token audit was done by script rather than by eye: parsed the front matter and diffed it
  against `tokens.css`. 0 mismatches across `primary/secondary/muted/neutral/surface/sunken/
  outline/divider/accent/accent-pressed/accent-tint/on-accent` and their 12 `dark-*`
  counterparts mapped onto the same variable names in `:root[data-theme="dark"]`; spacing
  `xs 4 / sm 8 / md 16 / lg 24 / xl 32 / xxl 48 / xxxl 64` all match. The only hex values
  outside the doc palette are `--qr-ink` / `--qr-paper`, which deliberately keep the pairing
  panel dark-on-white in both themes so the code stays scannable.
- No raw hex anywhere outside `tokens.css` (`css/app.css`, `share.html`, JS modules all read
  variables only).
- Console: `Page.addScriptToEvaluateOnNewDocument` captured `error`, `unhandledrejection` and
  `console.error` from before module evaluation; zero entries on all ten routes and on
  `share.html`.
- Outcome: no overflow, no token drift, no console errors — so no code was changed. Port 8000
  was free before this run; the `python3 -m http.server` started for the sweep was stopped.

### Notes for Phase 2 (review)
- Known gaps are listed at the end of `clients/web/README.md` (mock QR grid, preset batches
  instead of a file picker, Inter not bundled, library support-line copy vs. seeded count).
- States that need a peer (`session.offered`) are reachable with `?incoming=1` or
  `dropgrab.simulateIncoming()`; empty nearby with `dropgrab.setDevices([])`.

---

## Phase 2: Code review and verification
**Owner:** Code reviewer agent
**Status:** COMPLETE — 2026-11-18

### Task
Review the frontend against the design docs, verify it runs, and fix defects.

### Todo
- [x] Check token values and contrast against the design doc
- [x] Check left-aligned text, Lucide-only icons, 4/8 spacing, no placeholder copy
- [x] Check all screens listed in Phase 1 exist and render
- [x] Run and browser-test; fix issues found
- [x] Record findings and corrections below

### Completed
- Token audit (script, `css/tokens.css` vs the YAML front matter): all 12 light and 12 dark
  colour values equal the doc; spacing 4/8/16/24/32/48/64 equals the doc; type sizes and line
  heights equal the typography block. Contrast recomputed with WCAG relative luminance for all
  21 documented pairs in both themes: every pair matches the doc's measured ratio (±0.05) and
  clears 4.5:1 (lowest: `outline` on `sunken`, 4.94 light / 4.66 dark). A live DOM sweep of every
  rendered text node on all ten routes found 0 below-threshold pairs in light and dark, including
  with each sheet open; the toast pair (`primary` fill, `neutral` text) is 18.2:1 light and
  17.2:1 dark.
- Wording/layout audit: no centered text blocks (only tab-bar labels, segmented-control labels
  and single-word fixed-width controls, which the doc explicitly allows), all headline/body/
  metadata text is start-aligned, every icon comes from `js/icons.js` (Lucide, stroke 2,
  `currentColor`; the only other SVG is the QR module grid, which is a graphic, not an icon), and
  no raw hex outside `tokens.css`. Spacing scan of `app.css` found no off-scale lengths (only
  breakpoint widths, 1 px borders, 2 px rings and the 20 px select-caret origin). No lorem ipsum,
  "Item 1", "Device A", "Sample file", "Oops"/"Something went wrong", or exclamation marks in UI
  strings.
- Screens: `#/first-run`, `#/nearby`, `#/downloads`, `#/quality`, `#/browser`, `#/library`,
  `#/devices`, `#/settings`, `#/qr`, `#/transfer` and `share.html` all render, with no console
  errors or unhandled rejections on a fresh load in both themes. All sheets/dialogs render:
  choose, incoming, visibility, device detail, forget, detected items, delete, download menu,
  device name.
- Control API: every command and event listed in `dropgrab-app-design.md` §3.5 now exists in
  `js/mock/backend.js` (script diff: 0 missing commands, 0 missing events; the mock's extra
  `settings.changed`, `pair.updated`, `webshare.*` events are documented in the README). Every
  method the UI calls is in the doc's command list.
- Browser-tested flows (Chromium, cache disabled, 1440×900 and 390×844): first run → Nearby;
  choose 2/3 files → tap device → transfer → done; incoming request → accept → receiving;
  media link → quality (chips on compact, radios on wide) → download; direct link; browser page →
  detected items → video quality and PDF direct; download pause/resume and overflow menu; QR
  scan → 4-digit check → trust; library select → delete; device detail → forget; settings theme,
  text size and device name; web-share code entry (504193) and file list. 390 px overflow is 0
  on every route, with the choose and incoming sheets open, and at Largest text.

### Findings and corrections

**High — media downloads downloaded the wrong file**
- `downloads.add` ignored the requested title and always created
  `Rust ownership explained.mp4`; the quality screen also dropped the URL, so a Lecture link or
  the detected "Lecture 07 Operating Systems" item downloaded the Rust file (and a PDF went
  through the video quality picker).
- Fixed in `js/mock/backend.js` (accept `params.name`, fall back to the URL's file name),
  `js/screens/quality.js` (send the item's name, keep its URL), `js/screens/downloads.js` (keep
  the probed URL on the quality item) and `js/ui/sheets.js` (non-video detected items download
  directly as `<name>.pdf`; video items keep a page URL). Verified: Lecture downloads as
  "Lecture 07 Operating Systems.mp4", slides as "Chapter 4 slides.pdf".

**High — Nearby lied about the selected state and its primary button**
- After choosing files, every device tile rendered as selected (`accent-tint` + check icon) and
  the primary button changed to "Send N files" but still reopened the file chooser.
- Fixed in `js/screens/nearby.js`: tiles keep the send icon/state until a device is chosen, and
  the button stays "Choose files" (the selection banner already says "Choose a device to send
  to"). Verified with a 3-file selection: 0 `is-target` tiles, correct label, tap-to-send works.

**Medium — a probing download looked paused and offered Resume**
- A new job is created in `probing` for ~0.9 s, but the card rendered "Paused at 0% · 0 MB of
  198 MB" with a Resume button.
- Fixed in `js/screens/downloads.js`: probing renders "Checking the link", with no progress bar,
  percentage or action; the normal downloading card follows. Verified before and after the 900 ms
  transition.

**Medium — focus never returned from sheets, and could sit behind the incoming modal**
- `closeSheet()` checked `document.contains(lastFocused)` after `render()` had replaced the DOM,
  so the trigger (always detached) was never refocused; a `session.offered` sheet also left focus
  on the screen behind the scrim.
- Fixed in `js/main.js`: `openSheet` stores a focus snapshot (action/id/focus-id) and
  `closeSheet` restores it; a sheet that appears from an event now receives focus on first paint;
  if the incoming request refuses to close, focus stays inside it. Verified: Escape on the
  visibility sheet returns focus to the visibility button; Escape on the incoming request keeps it
  open with focus inside.

**Medium — "Text from the clipboard" sent to a device without asking**
- The chooser hardcoded `dev-maya-mba` and sent immediately, skipping the "pick a device" step.
- Fixed in `js/ui/sheets.js` / `js/screens/nearby.js`: choosing clipboard text sets a pending
  text and returns to Nearby with "Choose a device to send the text to"; tapping a tile sends it
  (and names the device). Choosing files clears the pending text.

**Low — control API gaps**
- `extractor.update`, `extractor.sites` and `backend.shutdown` were missing, and `peer.up` /
  `peer.down` were never emitted. Added the three commands and made `setDevices()` diff the list
  and emit `peer.up` / `peer.down` before `resync`. `README.md` command list updated to match.
  Verified the empty-nearby hook emits five `peer.down` events and restoration emits `peer.up`.

**Low — touch targets and alignment**
- The Nearby visibility line was a 32 px-tall button; set to `min-height: 48px`
  (`js/screens/nearby.js`). The device-card header hit area was 28 px tall; added
  `min-height: 48px` (`css/app.css`). `.btn-sm` was 40 px (used by banner and selection-bar
  actions); changed to 48 px so buttons meet the doc's 48 px target (chips stay 40 px as
  specified). The quiet "Scan a QR code" button was inset 8 px by its own padding; set
  `padding: 0` so it aligns with the device tiles.

**Low — empty link intake downloaded a hardcoded file**
- Pressing Download with an empty link field started `report.pdf`; it now shows "Paste a link to
  a file, video, or song" and does nothing (`js/screens/downloads.js`).

### Open items (not changed)
- Library support line is the doc's sample "128 files · 14.2 GB" while six items are seeded (already
  a documented known gap in `README.md`).
- QR image is a decorative module grid, Inter is not bundled, "Choose what to send" uses preset
  batches — all pre-existing, documented known gaps.
- Transfer failure (`Couldn't finish sending…`), unknown-device incoming and executable-file
  banners exist in the design but no mock path triggers them; they are unreachable states, not
  broken ones. No code change made.

### Progress log
- 2026-11-18: Read Phase 1 notes, both design docs, and all client source. Reused nothing running;
  started `python3 -m http.server 8000` in `clients/web` for the review and stopped it at the end.
- 2026-11-18: Scripted token/contrast audit against the design YAML; 0 mismatches, 0 failing
  pairs. Scripted API-name diff against §3.5: 3 commands and 2 events missing (fixed).
- 2026-11-18: Ran every route and sheet in Chromium at 1440 and 390 px, light and dark, with
  Largest text; 0 console errors, 0 horizontal overflow, 0 live contrast failures.
- 2026-11-18: Exercised the flows listed under Completed; found and fixed the defects above.
  Re-ran the full route sweep and the key flows after the fixes (cache disabled) with no errors.
- 2026-11-18: Files changed: `js/main.js`, `js/mock/backend.js`, `js/screens/nearby.js`,
  `js/screens/downloads.js`, `js/screens/quality.js`, `js/ui/sheets.js`, `css/app.css`,
  `README.md`. No Phase 1 content was edited.

---

## Phase 3: Backend build
**Owner:** Backend agent
**Status:** IN PROGRESS

### Task
Build the DropGrab backend from `dropgrab-app-design.md` (sections 3.5 control API, 4 transfer,
5 downloads, 6 integration, 7 library, plus discovery and web-share), so that the mock contract in
`clients/web/js/mock/backend.js` can be swapped for the real engine. Work only inside `Dropgrap/`.

### Stack decision
- The design names Rust (section 3.3), tokio, axum, rustls, rusqlite, reqwest-class HTTP client.
  The host had no Rust toolchain; installed `rustup` stable (rustc 1.100.0) under `~/.cargo`.
  Crates.io is reachable, so the design's stack is used.
- Layout: `backend/` Cargo workspace with three crates:
  - `dg-types`: command/event name lists (§3.5), error codes, wire types for the peer protocol.
  - `dg-core`: engine, control API, discovery, peer HTTPS protocol, sessions, downloads, library,
    devices, settings, pairing, web share.
  - `dg-daemon`: `dropgrabd` binary (control API on loopback, peer HTTPS listener, discovery).

### Todo
- [x] Confirm toolchain (installed rustup stable)
- [ ] Workspace, crates, README
- [ ] Control API (JSON-RPC over HTTP POST, SSE events with seq + replay + resync, token, Host/Origin checks)
- [ ] Settings, devices/trust, visibility
- [ ] Discovery (multicast announce, peer table, peer.up/peer.down)
- [ ] Peer HTTPS protocol with fingerprint pinning (offer/accept, PUT ranges, status, complete, cancel)
- [ ] Session send/receive, pause/resume/cancel, history, text
- [ ] Pairing (QR payload, PIN, verification code)
- [ ] Direct download engine (probe, segments, Range resume, pause/resume/cancel, queue)
- [ ] Extractor host (no signed plugin host in this build: resolve returns EXTRACT_FAILED)
- [ ] Library (query/get/delete/move/duplicates) + SQLite schema from §7.1
- [x] Web share (HTTPS, PIN, expiry) — hardened 2026-11-18, see log
- [ ] Remote download request
- [x] Tests: control-API contract (every §3.5 name), transfer e2e, download e2e, webshare e2e
- [~] Run tests and daemon for real, record output (workspace tests run; daemon smoke run not yet done)

### Progress log
- Read both design docs, the mock backend, and the UI calls (`invoke` sites) to fix payload shapes.
- Installed Rust stable 1.100.0 via rustup (`~/.cargo`). Workspace compiles with axum 0.8,
  axum-server 0.8 (rustls, no-provider, ring), reqwest 0.12 (rustls, ring), rcgen 0.13, rusqlite 0.40
  (bundled SQLite), tokio 1.
- Decisions (deviations from the design are listed here so they are not hidden):
  - Fingerprint on the wire: full SHA-256 of the certificate DER, lowercase hex (pinning compares the
    full digest). Display form: first 8 groups of 4 hex characters, uppercase (section 4.1 wording).
  - Verification code: first 4 bytes of SHA-256 over the two sorted fingerprints, mod 10000, 4 digits.
  - Control API transport: loopback TCP with a per-install bearer token (section 3.5 optional mode),
    not a Unix socket. Host and Origin checks as specified. Unix socket is not implemented yet.
  - SQLite: one connection behind a mutex instead of a writer task plus read pool. Same schema as 7.1.
  - Extractor: no signed WASM plugin host in this build. A generic resolver returns a direct
    audio/video URL as one format. Pages with no matching plugin return EXTRACT_FAILED.
  - Peer sender identity is bound by mutual TLS (2026-11-18, see the mTLS log entry below). The
    offer's `sender.fingerprint` must match the client certificate presented on the handshake.
  - Services are mutex-guarded structs inside one `Inner` (not actor tasks with bounded channels).
    Event bus is a bounded replay ring (1,000 events) plus a broadcast channel.
  - Download segments are fixed at plan time (no work stealing yet); per-host cap 8.
  - Download job states use the UI names: queued, probing, downloading, paused, finished, failed,
    cancelled. Session states: offered, transferring, paused, done, cancelled, declined,
    timed-out, failed.
- 2026-11-18 (backend agent, review pass): stale-state audit found the listed features built but
  web share had defects against §4.8. Fixed in `crates/dg-core/src/webshare.rs`:
  - expiry timer called `stop_inner`, which stopped whatever share was current, so an old share's
    timer could kill a newer share. Timer now stops only its own share (`stop_matching`).
  - no PIN attempt limit. Added `MAX_PIN_ATTEMPTS = 5`; after 5 wrong codes unlock returns 429.
  - downloads read whole files into memory. Now streamed with `ReaderStream` and Content-Length.
  - dead `engine` field in `ShareState` (the build warning) removed.
  Added `crates/dg-core/tests/webshare_e2e.rs` (2 tests). Verified the stale-timer test FAILS with
  the old `stop_matching(&eng, None)` line and PASSES with the fix.
  `cargo test --workspace --offline`: 21 unit + 6 control-API + 3 download + 2 transfer + 2 webshare
  + 3 types = 37 passing, 0 failing; `cargo build` has 0 warnings (was 1).
- Known open items: daemon (`dropgrabd`) not yet smoke-run end to end; no Unix socket transport;
  HLS/DASH not implemented.

### Mutual TLS for peer identity (backend agent, 2026-11-18)
- Problem: auto-accept trusted the `sender.fingerprint` a peer claimed in its offer, so any device
  could claim a trusted device's fingerprint.
- Change: the peer listener requests a client certificate (`identity::PeerCertVerifier`, optional,
  proof of possession via the handshake signature). `peer::MtlsAcceptor` records the SHA-256 of
  the presented certificate for each connection and adds it to request extensions. Handlers return
  403 unless that fingerprint equals the claimed one: `announce`, `session/offer`, `text`,
  `download-request`, `pair`. Session routes (`file/*`, `status`, `complete`, `cancel`) require the
  session's own peer certificate.
- Clients: peer-protocol calls use `identity::mtls_client` (presents this install's cert): session
  send/driver/cancel, `text.send`, `remote.request_download`, `pair.qr_scan`. `pinned_client`
  (no client cert) stays for `info` and web share.
- Decision: client certificates are accepted on proof of possession, not against a CA, to match
  the pinned-certificate model. The stored device fingerprint is the trust anchor, so no new
  trust-store format was needed.
- Tests: `tests/mtls_e2e.rs` (5 tests). Mismatched cert, absent cert, spoofed text sender and a
  non-participant cancel are refused with no event or session created. Each refusal test was run
  with the claimed-fingerprint check disabled (temporary env hook, removed afterwards) and FAILED;
  the matching-cert control passes either way.
- Results: `cargo test --workspace --offline`: 42 passing (21 unit + 6 control API + 3 download +
  5 mTLS + 2 transfer + 2 webshare + 3 types), 0 failing. `cargo build --offline`: 0 warnings.

### Review fixes: extractor truncation, download slot leak, pairing PIN lockout (backend agent, 2026-11-18)
Three blocking defects from review, each with a regression test that was run against the old code
first to confirm it fails there.

1. **Extractor panic on a non-char boundary** (`crates/dg-core/src/extractor.rs`).
   `body[..MAX_HTML]` panicked when the 2 MiB index landed inside a multi-byte UTF-8 character,
   and `Response::text()` buffered the whole page first.
   - `read_html_body` now reads the body in chunks and stops at `MAX_HTML` bytes, so an oversized
     page is never buffered in full.
   - `truncate_on_char_boundary` floors the cut to the previous character boundary and is applied
     after the capped read.
   - Tests: unit `truncation_never_splits_a_multibyte_char` and `body_read_stops_at_the_cap`
     (3 MiB server, asserts the reader returns exactly `MAX_HTML` bytes), plus
     `tests/extractor_e2e.rs` which resolves a ~6 MiB page whose cap offset is inside `€`.
     With the old line restored, the e2e test reproduces the original panic verbatim:
     `byte index 2097152 is not a char boundary; it is inside '€' (bytes 2097150..2097153)`;
     with the naive `truncate(max)` and with `resp.text()` the two unit tests fail
     (`body must be capped at MAX_HTML bytes`: left 6291456, right 2097152).

2. **Download slot leaked when a probe panicked** (`crates/dg-core/src/downloads.rs`).
   The slot was released by the code after `run_job(...).await`, which a panicking driver never
   reached, so the job stayed in `probing` and kept a concurrency slot forever.
   - `SlotGuard` (held by the driver task) releases `running`, marks a job left in `probing` /
     `downloading` as `failed` / `INTERNAL`, persists it and pumps the queue from `Drop`, so the
     release happens on normal completion, panic and abort alike.
   - `downloads.cancel` now finishes a job whose state is `probing` immediately (nothing has been
     written yet) and then pumps, and `finish_cancel` is idempotent via `Job::cancel_done` so the
     API call and the driver cannot emit `cancelled` twice. A cancel check was added after the
     part-file preallocation so a cancelled probe cannot resume into a preallocated file.
   - Tests (`tests/downloads_slot.rs`, `downloadsAtOnce = 1`): `panicking_probe_releases_the_slot_for_later_downloads`
     (probe panic → job `failed`/`INTERNAL`, next download reaches `finished`) and
     `cancel_frees_a_probing_job_and_its_slot` (job against a server that never answers is
     `probing`, cancel removes it, next download finishes). With the old spawn/cancel code both
     fail (`got Some("probing")`).
   - The panic is injected through `DROPGRAB_TEST_PANIC_PROBE` (URL substring) in
     `maybe_panic_probe`, which exists only in debug builds (`#[cfg(debug_assertions)]`) and is
     inert unless the variable is set; release builds compile the no-op variant.
     `cargo build --release --offline` was run to check that branch.

3. **Pairing PIN had no attempt limit** (`crates/dg-core/src/discovery.rs`).
   - `PairingState` gained `failed_pins`; `handle_pair` counts wrong PINs per pairing session and
     answers `429` once `MAX_PAIR_ATTEMPTS` (5) is reached, refusing even the correct PIN while
     locked. Because the counter lives in the session, `pair.qr_create` resets it — an attacker
     can only lock the code they are guessing at, and the user just generates a new one.
   - Tests (`tests/pairing_e2e.rs`): `five_wrong_pins_lock_the_session_and_a_new_code_resets_it`
     (4 × 403, 5th → 429, correct PIN → 429, new code accepts its PIN) and
     `a_few_wrong_pins_do_not_lock_the_session`. The first fails on the old code
     (`fifth wrong PIN must answer 429, got (403, ...)`).

- Results: `cargo test --workspace --offline`: **49 passing** (23 unit + 6 control API + 3 download +
  2 download slot + 5 mTLS + 1 extractor + 2 pairing + 2 transfer + 2 webshare + 3 types), 0 failing.
  `cargo build --offline`: 0 warnings; `cargo build --release --offline`: 0 warnings.
- Files changed: `crates/dg-core/src/extractor.rs`, `downloads.rs`, `discovery.rs`, `engine.rs`
  (PairingState field); new `crates/dg-core/tests/{extractor_e2e,downloads_slot,pairing_e2e}.rs`;
  `backend/README.md` test table. PROGRESS.md this entry.
- Still open: daemon smoke run, Unix socket control transport, HLS/DASH, and the pairing PIN is
  still a 6-digit code with a 2-minute TTL (the per-session limit now caps guessing at 5 tries).




## Frontend daemon API integration
**Owner:** Frontend integration subagent
**Status:** COMPLETE (browser transport has environment-dependent security constraints)

### Completed
- Added `clients/web/js/backend/client.js`: bearer-token JSON-RPC, SSE reconnect/replay cursor,
  event-driven state refresh and initial state loading over the backend's real methods.
- Added backend selection in `clients/web/js/backend/select.js`: mock remains default; configured
  token or `mode: real` enables daemon client.
- Fixed library send to use daemon-resolvable item IDs/paths; reject mock/dropped items lacking a
  daemon-readable path.
- Pair QR create uses the daemon URI; scan submits required `{uri}` (manual paste because there is
  no scanner integration). Remote download submits `{deviceId,url}`. Library Share calls
  `webshare.start` with selected library IDs and displays/stops the share.
- Documented real mode configuration, trust boundary and limitations in `clients/web/README.md`.
- Added Node tests in `clients/web/js/backend/client.test.mjs`.

### Verification
- `node --check` on modified JS and `node --test clients/web/js/backend/client.test.mjs`: **5 passing**.
- Served UI at `http://127.0.0.1:8022/`; requests for the index and new client module returned
  HTTP 200.
- Started `dropgrabd --no-discovery` on loopback port 51609 with isolated `/tmp/dropgrab-ui-*`
  directories. Authenticated real RPC output: `backend.info` → API 1.0, device “UI Integration
  Check”, peer port 37461; `devices.list` → empty `devices`; `settings.get` → live defaults
  including `downloadsAtOnce: 5`, `theme: system`, `visibility: everyone`. Token file mode was 0600.

### Remaining limitations
- No desktop-shell injector is provided. A normal webpage sends an `Origin` header, rejected by the
  daemon's empty origin allowlist. Configuration/injection is implemented, but browser-to-daemon
  requests require a trusted shell bridge or deliberate origin policy adjustment.
- Browser file drops cannot yield filesystem paths accepted by `session.send`; arbitrary local file
  transfers need a native file-picker bridge. Library item IDs can be sent.
- QR camera decode is absent; users paste the device URI.
- Existing unrelated backend working-tree modifications were not changed.

---

## Phase 3b: Browser origin policy, file staging, repo hygiene
**Owner:** Backend subagent
**Status:** COMPLETE — 2026-11-18

### Task
Unblock the web UI against the real daemon: (1) an explicit origin allowlist, (2) `files.stage`
so a browser can hand over file bytes, (3) `.gitignore` + untrack `backend/target`, (4) a live
daemon verification run. Docs updated; nothing committed.

### Todo
- [x] `--allow-origin <origin>` (repeatable) on `dropgrabd`; default stays deny-all
- [x] CORS headers + `OPTIONS` preflight for allowed origins (needed by a real browser)
- [x] `files.stage` RPC: base64 content, size cap, name validation, staging folder, id/path
- [x] `transfer.start` alias + `items[].stagedId`; cleanup on transfer end and by TTL
- [x] Tests: origin policy, staging round trip, traversal, oversize, TTL, body cap
- [x] `.gitignore` at root and in `backend/`; `git rm -r --cached backend/target`
- [x] Live run of `dropgrabd` with and without the flag

### API added (shapes the frontend can use)
- Method `files.stage`:
  request `{ "name": "photo.jpg", "content": "<base64>", "mime": "image/jpeg" }`
  response `{ "id": "stg_<32 hex>", "name": "photo.jpg", "size": 184320, "path": "<data-dir>/staging/<id>/photo.jpg" }`.
  Errors: `INVALID_PARAMS` (missing name/content, separator or `..`/`.` in the name, name > 200
  chars, bad base64), `PAYLOAD_TOO_LARGE` (decoded file > 8 MiB, message
  `file is larger than the 8 MiB staging limit`).
- Method `transfer.start` (alias of `session.send`, same params): `{ "peer": "<deviceId>",
  "items": [ { "stagedId": "stg_..." } ] }` → `{ "sid": "<uuid>", "state": "offered" }`;
  items may instead carry `{ "path": "<staging path>" }`, a library `{ "id": ... }`, or a plain
  `{ "path": ... }`. Unknown `stagedId` → `NOT_FOUND`.
- Flag `--allow-origin <origin>` (repeatable, `--allow-origin=<origin>` also accepted). An
  allowed origin calling `/rpc` or `/events` with a valid token gets 200 plus
  `Access-Control-Allow-Origin` and `Vary: Origin`; `OPTIONS` preflight answers 204 with
  `Access-Control-Allow-Methods/Headers`. Unlisted origin → 403, no CORS headers. No `Origin`
  header → unchanged (token only). No settings key: the policy is set at launch, so a leaked
  token cannot widen it through `settings.set`.

### Implementation
- `crates/dg-core/src/staging.rs` (new): `files.stage`, name validation (`stage_name`),
  8 MiB decoded cap (`MAX_STAGE_BYTES`, same as `peer::MAX_CHUNK`), `<data-dir>/staging/<id>/`,
  registry in `Engine.staging`, `release` on session end, `sweep` (1 h TTL, scanned every 10 s
  from the pump), startup orphan sweep.
- `transfer.rs`: `Session.staged`, `resolve_item` accepts `stagedId`, `finish` releases staged
  ids (done, cancelled and failed all pass through `finish`).
- `api.rs`: `serve_with_origins` (old `serve` keeps the deny-all default), `normalize_origin`,
  origin allowlist check with trailing-slash tolerance, `cors_origin`/`with_cors`/`preflight`,
  `/rpc` body cap raised to carry base64 up to the staging cap while every other method keeps
  64 KiB (413 otherwise).
- `dg-types`: `EXTENSION_COMMANDS` = `session.list`, `files.stage`, `transfer.start`.
- `dg-daemon`: `--allow-origin` parsing/validation (`scheme://host[:port]`, no path).
- `base64 0.22` added to `dg-core` (already in `Cargo.lock`; builds offline).

### Tests (all real runs)
`crates/dg-core/tests/origin_policy.rs` (7): allowed origin accepted + ACAO header; sibling
port on `localhost` 403; unlisted origins (`http://evil.example`, `null`, `https://localhost:8080`)
403 with a valid token; no-Origin request 200 and no ACAO; bad/missing token 401 for an allowed
origin; default `serve` rejects every browser origin; preflight 204/403 follows the allowlist;
origin normalisation.
`crates/dg-core/tests/files_stage.rs` (6): stage 200 KiB → `transfer.start` by `stagedId` →
accept → done → bytes identical on the receiver → staged file and folder gone; same file sent by
`path`; unknown `stagedId` and a traversal-looking id → `NOT_FOUND`; names
`../../evil.txt`, `..\..\evil.txt`, `a/b.txt`, `..`, `.`, `/etc/passwd` → `INVALID_PARAMS` with
nothing created outside `staging/`; oversize (8 MiB + 1) → `PAYLOAD_TOO_LARGE`, exactly 8 MiB
accepted; TTL sweep removes an aged entry; over HTTP a 200 KiB `files.stage` body is 200 while
the same-size `settings.set` body is 413.
Unit tests in `staging.rs` (2): traversal names rejected, ordinary names kept/sanitized.

Result: `cargo test --workspace --offline` → **64 passing, 0 failing** (25 unit + 6 control API +
3 download + 2 download slot + 1 extractor + 6 files.stage + 5 mTLS + 7 origin + 2 pairing +
2 transfer + 2 webshare + 3 types). `cargo build --workspace --offline` → 0 warnings.

### Repo hygiene
- Added `/Dropgrap/.gitignore` and `/Dropgrap/backend/.gitignore` (`target/`, `**/target/`,
  `*.rs.bk`, logs, editor/OS files).
- `git rm -r --cached backend/target` (files kept on disk: 8.1 GB still present, nothing deleted).
- `git status --short` line count: **230 at the start of this task** (target churn dominating),
  **67 immediately before the untracking** (after the rebuild settled), **3557 after** — of which
  3525 lines are the one-time staged `D` entries for the now-ignored build files, and 32 are the
  real source/docs changes. Tracked files under `backend/target`: 3525 → **0**. `git check-ignore`
  reports `backend/target/debug` as ignored. Nothing was committed.

### Live verification (real daemon, real HTTP)
Built `target/debug/dropgrabd`, `--no-discovery`, ephemeral control/peer ports, isolated
`/tmp/dg-live-*` data dirs; the token came from `--print-token`.

Without `--allow-origin` (default deny-all):
```
backend.info (no Origin)                     HTTP 200  {"apiVersion":"1.0","version":"dropgrabd 1.0.0","port":...}
backend.info (Origin: http://localhost:8080) HTTP 403  "origin not allowed"
devices.list (no Origin)                     HTTP 200  {"devices":[]}
settings.get (no Origin)                     HTTP 200  settings object (deviceName, visibility "everyone", downloadsAtOnce 5, theme "system", ...)
backend.info (bad token)                     HTTP 401  "missing or wrong token"
OPTIONS preflight (Origin: localhost:8080)   HTTP 403  "origin not allowed"
files.stage (no Origin)                      HTTP 200  {"id":"stg_b62a558e...","name":"hello.txt","size":111,"path":".../data/staging/stg_b62a.../hello.txt"}
```

With `--allow-origin http://localhost:8080`:
```
backend.info (allowed Origin)                HTTP 200  ACAO=http://localhost:8080
backend.info (Origin: http://evil.example)   HTTP 403  no CORS headers
backend.info (no Origin)                     HTTP 200
backend.info (allowed Origin, bad token)     HTTP 401  ACAO set
OPTIONS preflight (allowed Origin)           HTTP 204  ACAO=http://localhost:8080, allow-headers="authorization, content-type, last-event-id"
files.stage (allowed Origin)                 HTTP 200  staged id/name/size/path returned, bytes match, path inside <data-dir>/staging
files.stage name "../../evil.txt"            HTTP 200  {"error":{"code":"INVALID_PARAMS","message":"name must not contain a path separator (send a file name, not a path)"}}
```
Both daemons were stopped (exit 0, `dropgrabd: stopped`) and their temp dirs removed.

### Files changed
`backend/crates/dg-core/src/{staging.rs (new), api.rs, engine.rs, lib.rs, transfer.rs}`,
`backend/crates/dg-core/tests/{files_stage.rs (new), origin_policy.rs (new)}`,
`backend/crates/dg-core/Cargo.toml`, `backend/crates/dg-types/src/lib.rs`,
`backend/crates/dg-daemon/src/main.rs`, `backend/Cargo.lock` (base64 edge),
`backend/README.md`, `Dropgrap/.gitignore` (new), `backend/.gitignore` (new), `PROGRESS.md`.

### Still open
- The web client must still be served from an origin passed to `--allow-origin` and must put the
  token in the page (there is no native shell bridge); file drops now work through `files.stage`
  but the client side is not wired yet.
- Staged files are not persisted across restarts (registry is in memory); leftovers are swept.


## Web client real-mode file staging and QR scan
**Owner:** Frontend agent
**Status:** IMPLEMENTED — 2026-11-18

- [x] Read browser-picked and dropped `File` objects as base64; call `files.stage` then
      `transfer.start` with each returned `stagedId`.
- [x] Keep library IDs and trusted daemon paths on the existing send path.
- [x] Add camera QR scan using `BarcodeDetector` when available, plus URI paste fallback and
      visible messages for the camera and fallback cases.
- [x] Document `--allow-origin`, token sources/configuration, and that a native desktop shell
      bridge is not part of this change.
- [x] Verify JavaScript syntax, web client unit tests, and a real daemon stage/send RPC run.

### Verification
- `node --check` passed for `js/screens/nearby.js`, `js/screens/qr.js`, `js/ui/sheets.js`,
  and `js/main.js`.
- `node --test js/backend/client.test.mjs`: 6 passing, 0 failing (includes base64 byte encoding).
- Built daemon with `cargo build --offline -p dg-daemon`; launched it on control port 58847,
  peer port 0, `--no-discovery`, and `--allow-origin http://localhost:8080`.
- Real authenticated `files.stage` returned `{ id: "stg_a9d58abfff083fc5bea289536212aefd",
  name: "web-client-e2e.txt", size: 26, path: "/home/kali/.hermes/cache/scratch/dropgrab-web-e2e/staging/stg_a9d58abfff083fc5bea289536212aefd/web-client-e2e.txt" }`.
  `transfer.start` with this staged id and a deliberately nonexistent peer returned the real
  daemon error `{ code: "NET_UNREACHABLE", message: "device is not reachable on this network" }`.
  This verifies stage and request handling, but no peer was available to reach the `offered` state.
  `transfer.start` responses now update backend state like `session.send`.
- No native shell bridge is included; browser camera hardware flow was not exercised in an actual
  camera-enabled browser.
