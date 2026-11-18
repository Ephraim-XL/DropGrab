// DropGrab web client: router, state, actions, and rendering.

import { backend, REAL_BACKEND } from './backend/select.js';
import { navHtml, applyTheme, watchSystemTheme } from './ui/shell.js';
import { escapeHtml } from './ui/dom.js';
import { renderSheet, actions as sheetActions } from './ui/sheets.js';
import * as firstRun from './screens/first-run.js';
import * as nearby from './screens/nearby.js';
import * as transfer from './screens/transfer.js';
import * as qr from './screens/qr.js';
import * as downloads from './screens/downloads.js';
import * as quality from './screens/quality.js';
import * as browser from './screens/browser.js';
import * as library from './screens/library.js';
import * as devices from './screens/devices.js';
import * as settings from './screens/settings.js';

const SCREENS = {
  'first-run': firstRun,
  nearby,
  transfer,
  qr,
  downloads,
  quality,
  browser,
  library,
  devices,
  settings,
};

const ROUTE_GROUP = {
  transfer: 'nearby',
  qr: 'nearby',
  quality: 'downloads',
};

const ACTION_HANDLERS = Object.assign(
  {},
  sheetActions,
  firstRun.actions,
  nearby.actions,
  transfer.actions,
  qr.actions,
  downloads.actions,
  quality.actions,
  browser.actions,
  library.actions,
  devices.actions,
  settings.actions,
);

const root = document.getElementById('root');

const app = {
  route: 'nearby',
  ui: {
    sheet: null,
    selection: [],
    pickTarget: false,
    pendingDeviceId: null,
    pendingText: null,
    linkUrl: '',
    clipboardBanner: true,
    quality: 'balanced',
    qualityItem: null,
    libraryFilter: 'all',
    librarySearch: '',
    selectedIds: [],
    deleteIds: [],
    deviceId: null,
    downloadId: null,
    alwaysAccept: false,
    qrStep: 'idle',
    qrPeer: null,
    qrTrust: false,
    browserUrl: 'example.org/lectures',
    toast: null,
  },
};

let renderTimer = null;
let toastTimer = null;
let lastFocus = null;

/* ---------- rendering ---------- */

function makeCtx() {
  return {
    s: backend.state,
    ui: app.ui,
    app,
    backend,
    realBackend: REAL_BACKEND,
    invoke: (method, params) => backend.invoke(method, params),
    go,
    openSheet,
    closeSheet,
    toast,
    render,
  };
}

function captureFocus() {
  const el = document.activeElement;
  if (!el || el === document.body) return null;
  return {
    focusId: el.dataset ? el.dataset.focusId : null,
    action: el.dataset ? el.dataset.action : null,
    id: el.dataset ? el.dataset.id || el.dataset.deviceId : null,
    label: el.getAttribute ? el.getAttribute('aria-label') : null,
    inSheet: !!el.closest('.sheet'),
  };
}

function restoreFocus(snapshot) {
  if (!snapshot) return;
  let next = null;
  if (snapshot.focusId) {
    next = document.querySelector(`[data-focus-id="${CSS.escape(snapshot.focusId)}"]`);
  }
  if (!next && snapshot.action) {
    const scope = snapshot.inSheet ? document.querySelector('.sheet') : document;
    const selector = snapshot.id
      ? `[data-action="${snapshot.action}"][data-id="${snapshot.id}"]`
      : `[data-action="${snapshot.action}"]`;
    next = (scope || document).querySelector(selector) || document.querySelector(selector);
  }
  if (!next && snapshot.label) {
    next = document.querySelector(`[aria-label="${CSS.escape(snapshot.label)}"]`);
  }
  if (next) next.focus({ preventScroll: true });
}

function render() {
  const focus = captureFocus();
  const s = backend.state;
  applyTheme(s.settings);

  const route = SCREENS[app.route] ? app.route : 'nearby';
  const screen = SCREENS[route];
  const navRoute = ROUTE_GROUP[route] || route;
  const ctx = makeCtx();

  const sheet = renderSheet(ctx);
  const hadSheet = !!root.querySelector('.sheet');
  const toastHtml = app.ui.toast
    ? `<div class="toast" role="status"><span>${escapeHtml(app.ui.toast.message)}</span></div>`
    : '';

  root.className = route === 'first-run' ? 'app no-nav' : 'app';
  root.innerHTML = `
    ${navHtml(navRoute)}
    <main>
      <div class="content" id="screen-content">${screen.render(ctx)}</div>
    </main>
    ${sheet}
    ${toastHtml}
  `;

  restoreFocus(focus);

  const sheetEl = document.querySelector('.sheet');
  if (sheetEl && !hadSheet) {
    const first = sheetEl.querySelector('[data-action], input, select, button');
    if (first) first.focus({ preventScroll: true });
  }
}

function scheduleRender() {
  if (renderTimer) return;
  renderTimer = setTimeout(() => {
    renderTimer = null;
    render();
  }, 80);
}

/* ---------- navigation and overlays ---------- */

function go(route) {
  if (!SCREENS[route]) route = 'nearby';
  app.route = route;
  app.ui.sheet = null;
  if (location.hash !== `#/${route}`) {
    history.pushState(null, '', `#/${route}`);
  }
  render();
  window.scrollTo({ top: 0, behavior: 'auto' });

  if (route === 'qr' && !backend.state.pairing) {
    backend.invoke('pair.qr_create', {}).then((pairing) => {
      backend.state.pairing = pairing;
      scheduleRender();
    }).catch((error) => {
      console.error(error);
      toast('Could not create a pairing code.');
    });
  }
}

function openSheet(name) {
  lastFocus = captureFocus();
  app.ui.sheet = name;
  render();
  const sheetEl = document.querySelector('.sheet');
  if (sheetEl) {
    const first = sheetEl.querySelector('[data-action], input, select, button');
    if (first) first.focus({ preventScroll: true });
  }
}

function closeSheet() {
  app.ui.sheet = null;
  render();
  const stillOpen = document.querySelector('.sheet');
  if (stillOpen) {
    const first = stillOpen.querySelector('[data-action], input, select, button');
    if (first) first.focus({ preventScroll: true });
  } else if (lastFocus) {
    restoreFocus(lastFocus);
  }
  lastFocus = null;
}

function toast(message) {
  app.ui.toast = { message, at: Date.now() };
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    app.ui.toast = null;
    render();
  }, 4000);
  render();
}

/* ---------- events ---------- */

backend.subscribe((event) => {
  if (event.type === 'notification.request') {
    const data = event.data || {};
    if (data.kind !== 'incoming') toast(data.body ? `${data.title}. ${data.body}` : data.title);
  }
  if (event.type === 'error') toast('A backend error was reported');
  if (event.type === 'resync') {
    app.ui.selectedIds = [];
    app.ui.selection = [];
  }
  scheduleRender();
});

/* ---------- input handling ---------- */

document.addEventListener('input', (event) => {
  const el = event.target;
  const model = el.dataset ? el.dataset.model : null;
  if (!model) return;
  if (model === 'link') app.ui.linkUrl = el.value;
  else if (model === 'librarySearch') app.ui.librarySearch = el.value;
  else if (model === 'browserUrl') app.ui.browserUrl = el.value;
  else if (model === 'deviceName') app.ui.deviceNameDraft = el.value;
  // restore focus after the render triggered below
  const focusId = el.dataset.focusId;
  const caret = el.selectionStart;
  render();
  const next = focusId && document.querySelector(`[data-focus-id="${CSS.escape(focusId)}"]`);
  if (next) {
    next.focus({ preventScroll: true });
    if (caret != null && next.setSelectionRange) next.setSelectionRange(caret, caret);
  }
});

document.addEventListener('change', async (event) => {
  const el = event.target;
  if (el.id === 'browser-file-picker') {
    const files = [...(el.files || [])];
    if (files.length) {
      app.ui.selection = files.map((file, index) => ({
        id: `picked-${index}-${file.name}`,
        name: file.name,
        kind: kindFromMime(file.type),
        size: file.size,
        file,
      }));
      render();
    }
    return;
  }
  const setting = el.dataset ? el.dataset.setting : null;
  if (!setting) return;
  const value = el.type === 'checkbox' ? el.checked : (el.type === 'number' ? Number(el.value) : el.value);
  await backend.invoke('settings.set', { [setting]: value });
  if (setting === 'visibility') backend.invoke('visibility.set', { visibility: value }).catch(console.error);
  render();
});

document.addEventListener('click', async (event) => {
  const nav = event.target.closest('[data-route]');
  if (nav) {
    event.preventDefault();
    go(nav.dataset.route);
    return;
  }
  const trigger = event.target.closest('[data-action]');
  if (!trigger) return;
  const action = trigger.dataset.action;
  const handler = ACTION_HANDLERS[action];
  if (!handler) return;
  event.preventDefault();
  try {
    await handler(makeCtx(), trigger, event);
  } catch (error) {
    console.error(error);
    toast('That action could not finish. Try again.');
  }
});

document.addEventListener('submit', (event) => {
  const form = event.target.closest('form[data-form]');
  if (!form) return;
  event.preventDefault();
  if (form.dataset.form === 'browser') {
    ACTION_HANDLERS['browser-go'](makeCtx());
  }
});

/* ---------- keyboard ---------- */

document.addEventListener('keydown', (event) => {
  const sheetOpen = !!document.querySelector('.sheet');
  const key = event.key;

  if (key === 'Escape') {
    if (sheetOpen && app.ui.sheet) {
      closeSheet();
      event.preventDefault();
    }
    return;
  }

  if (sheetOpen && key === 'Tab') {
    const focusables = [
      ...document.querySelectorAll(
        '.sheet button, .sheet [href], .sheet input, .sheet select, .sheet [tabindex]:not([tabindex="-1"])',
      ),
    ].filter((el) => !el.disabled);
    if (!focusables.length) return;
    const first = focusables[0];
    const last = focusables[focusables.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      last.focus();
      event.preventDefault();
    } else if (!event.shiftKey && document.activeElement === last) {
      first.focus();
      event.preventDefault();
    }
    return;
  }

  if (!(event.ctrlKey || event.metaKey)) return;
  const map = { '1': 'nearby', '2': 'downloads', '3': 'browser', '4': 'library', '5': 'settings' };
  if (map[key]) {
    event.preventDefault();
    go(map[key]);
  } else if (key.toLowerCase() === 'o') {
    event.preventDefault();
    go('nearby');
    openSheet('choose');
  }
});

/* ---------- drag and drop ---------- */

function kindFromMime(type = '') {
  if (type.startsWith('video/')) return 'video';
  if (type.startsWith('audio/')) return 'audio';
  if (type.startsWith('image/')) return 'image';
  if (/zip|archive|compressed/.test(type)) return 'archive';
  return 'document';
}

document.addEventListener('dragover', (event) => {
  if (event.dataTransfer && [...event.dataTransfer.types].includes('Files')) {
    event.preventDefault();
  }
});

document.addEventListener('drop', async (event) => {
  const tile = event.target.closest('.device-tile');
  const files = event.dataTransfer ? [...event.dataTransfer.files] : [];
  const text = event.dataTransfer ? event.dataTransfer.getData('text/uri-list') : '';
  if (files.length) {
    event.preventDefault();
    app.ui.selection = files.map((file, index) => ({
      id: `drop-${index}-${file.name}`,
      name: file.name,
      kind: kindFromMime(file.type),
      size: file.size,
      file,
    }));
    if (tile) {
      const { startSend } = await import('./screens/nearby.js');
      await startSend(makeCtx(), tile.dataset.deviceId);
    } else {
      toast(`${files.length} ${files.length === 1 ? 'file' : 'files'} ready. Choose a device.`);
      render();
    }
  } else if (text) {
    event.preventDefault();
    app.ui.linkUrl = text.trim();
    go('downloads');
    toast('Link ready. Press Download to start.');
  }
});

/* ---------- theme ---------- */

watchSystemTheme(backend.state.settings);

if (REAL_BACKEND) {
  backend.subscribe((event) => {
    if (event.type === 'session.offered') {
      backend.state.incoming = event.data;
      if (app.route !== 'transfer') app.ui.sheet = null;
    }
  });
  backend.initialize().then(() => {
    watchSystemTheme(backend.state.settings);
    render();
  }).catch((error) => {
    console.error('Could not connect to dropgrabd:', error);
    toast('Could not connect to DropGrab. Check the daemon and API configuration.');
  });
}

/* ---------- boot ---------- */

function initialRoute() {
  const hash = location.hash.replace(/^#\/?/, '');
  if (SCREENS[hash]) return hash;
  let done = false;
  try {
    done = localStorage.getItem('dropgrab.firstRun.v1') === 'done';
  } catch {
    done = false;
  }
  return done ? 'nearby' : 'first-run';
}

window.addEventListener('popstate', () => {
  const route = location.hash.replace(/^#\/?/, '');
  app.route = SCREENS[route] ? route : 'nearby';
  render();
});

app.route = initialRoute();
if (location.hash !== `#/${app.route}`) {
  history.replaceState(null, '', `#/${app.route}`);
}
render();

  if (new URLSearchParams(location.search).has('incoming') && !REAL_BACKEND) {
    setTimeout(() => backend.simulateIncoming(), 1200);
  }

// Demo hooks, for reviewing screens that a peer normally triggers.
window.dropgrab = {
  backend,
  invoke: (method, params) => backend.invoke(method, params),
  simulateIncoming: () => backend.simulateIncoming(),
  setDevices: (list) => backend.setDevices(list),
  go,
  get state() {
    return backend.state;
  },
  get ui() {
    return app.ui;
  },
};
