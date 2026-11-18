# DropGrab web client

The DropGrab UI as a dependency-free web client: plain HTML, CSS, and ES modules. It follows
`../../../dropgrab-ui-design.md` for tokens, components, screens, and copy, and
`../../../dropgrab-app-design.md` for the product context and the control API names.

There is no backend. A mock control API in `js/mock/` answers the same commands and emits the
same events the Rust engine would, so every screen is driven by state and events.

## Run it

ES modules do not load from `file://`, so serve the folder:

```sh
cd clients/web
python3 -m http.server 8000
```

Then open <http://localhost:8000/>. Opening `index.html` directly shows the same instructions
instead of a blank page.

No install step, no build step, no network requests. Everything (including the Lucide icons)
is local.

Reference captures are in `screenshots/`: `desktop-nearby-dark.png` (1440×900) and
`compact-downloads-light.png` (390×844 at 2×).

## Screens

| Route | Screen |
|---|---|
| `#/first-run` | First run |
| `#/nearby` | Nearby (home, the hero screen) |
| `#/downloads` | Downloads |
| `#/quality` | Choose a quality |
| `#/browser` | Browser |
| `#/library` | Library |
| `#/devices` | Devices (sidebar on expanded layouts) |
| `#/settings` | Settings |
| `#/qr` | Connect with a QR code |
| `#/transfer` | Transfer in progress, paused, or finished |

Sheets and dialogs: Choose what to send, Incoming request, Visibility, Device detail,
Forget/block, Download options, Detected items, Delete, Device name.

The web-share receiver page is a separate document: <http://localhost:8000/share.html>.
It has no navigation and its own 6-digit code entry (code `504193` unlocks the file list).
It is also linked from the bottom of the QR screen.

## Layout and theme

- Compact `< 600px`: bottom tab bar, 64 px.
- Medium `600–959px`: navigation rail, 80 px with labels under the icons.
- Expanded `≥ 960px`: 240 px sidebar with a Devices item; content max width 1120 px, left aligned.
- Theme follows `prefers-color-scheme`, with a manual System / Light / Dark override in
  Settings → Appearance. Text size offers Default, Large (125%), Largest (150%).
- All tokens live in `css/tokens.css`. No component reads a raw colour.

## Mock control API

`js/mock/backend.js` exposes `invoke(method, params)` and an event stream, with the command
names from `dropgrab-app-design.md` §3.5:

- Devices: `devices.list`, `devices.trust`, `devices.block`, `devices.forget`,
  `devices.set_allowance`, `visibility.set`
- Transfer: `session.send`, `session.respond`, `session.pause`, `session.resume`,
  `session.cancel`, `session.history`, `text.send`, `pair.qr_create`, `pair.qr_scan`
- Downloads: `downloads.add`, `downloads.probe`, `downloads.pause`, `downloads.resume`,
  `downloads.cancel`, `downloads.list`, `downloads.reorder`, `downloads.set_priority`
- Media: `extractor.resolve`, `extractor.update`, `extractor.sites`
- Library: `library.query`, `library.get`, `library.delete`, `library.move`, `library.duplicates`
- Remote and web share: `remote.request_download`, `remote.list_requests`,
  `webshare.start`, `webshare.stop`
- System: `settings.get`, `settings.set`, `backend.info`, `backend.shutdown`,
  `diagnostics.run`, `logs.export`

Events emitted: `peer.up`, `peer.down`, `session.offered`, `session.state`, `session.progress`,
`session.finished`, `download.state`, `download.progress`, `download.finished`,
`library.changed`, `device.changed`, `notification.request`, `error`, `resync`, plus
`settings.changed`, `pair.updated` and `webshare.*` for the mock.

Timers stand in for the engine: transfers and downloads advance while a job is active, so the
progress screens are live. Settings persist in `localStorage` under `dropgrab.settings.v1`;
first-run completion persists under `dropgrab.firstRun.v1`.

### Reviewing states that a peer normally triggers

```js
// In the browser console:
dropgrab.simulateIncoming()        // Maya's MacBook Air offers 3 files (662 MB)
dropgrab.setDevices([])            // nearby-empty state on the home screen
dropgrab.go('transfer')            // jump to a route
dropgrab.state                     // read the mock backend state
```

`http://localhost:8000/?incoming=1` opens the incoming request sheet automatically 1.2 s after
load, which is the quickest way to see the Accept → transfer flow.

## Keyboard

- `Ctrl/Cmd + 1…5` switch sections.
- `Ctrl/Cmd + O` opens Choose what to send.
- `Esc` closes a sheet or dialog (the incoming request must be accepted or declined).
- Drop files on a device tile to send; drop a link anywhere to start a download.

## Files

```
index.html          app shell, early theme script, file:// notice
share.html          standalone web-share receiver page
css/tokens.css      design tokens, typography scale, theme values
css/app.css         layout, components, breakpoints
js/icons.js         Lucide SVGs inlined from lucide-static 1.74.0 (ISC)
js/main.js          router, focus handling, actions, drag and drop
js/mock/backend.js  mock control API and event bus
js/mock/data.js     sample content (devices, files, downloads, library)
js/ui/*.js          shell, shared components, sheets
js/screens/*.js     one module per screen
js/webshare.js      receiver page logic
```

## Known gaps

- The QR image is a deterministic module grid drawn to look like a pairing code; it is not an
  encoder, so it does not carry a scannable payload.
- "Choose what to send" adds preset sample batches instead of opening an OS file picker.
  Dropping real files on the window or a tile does use their real names and sizes.
- Inter is not bundled; the system UI font is used unless Inter is installed locally.
- Library counts in the support line ("128 files · 14.2 GB") are the sample copy from the
  design document, while the list shows the six seeded items.
- Nothing is sent to a server: downloads and transfers are simulated, and the web-share page
  uses a fixed code.
