// Sheets and dialogs. One shell, one focus cycle, one Escape handler (in main.js).

import { icon } from '../icons.js';
import {
  deviceIcon,
  escapeHtml,
  fileIcon,
  fmtSize,
  switchControl,
  truncateName,
} from './dom.js';
import { SEND_BATCHES } from '../mock/data.js';

function shell({ title, body, actions = '', labelledBy = 'sheet-title', handle = true }) {
  return `<div class="scrim" data-action="scrim">
    <div class="sheet" role="dialog" aria-modal="true" aria-labelledby="${labelledBy}" tabindex="-1">
      ${handle ? '<div class="sheet-handle" aria-hidden="true"></div>' : ''}
      <h2 class="sheet-title" id="${labelledBy}">${title}</h2>
      <div class="sheet-body">${body}</div>
      ${actions ? `<div class="sheet-actions">${actions}</div>` : ''}
    </div>
  </div>`;
}

function fileList(files) {
  return files
    .map(
      (f) => `<div class="list-row">
        <span class="row-icon">${icon(fileIcon(f.kind), 24)}</span>
        <span class="row-body">
          <span class="row-title" title="${escapeHtml(f.name)}">${escapeHtml(truncateName(f.name))}</span>
        </span>
        <span class="row-trailing"><span class="t-body-sm ink-muted">${fmtSize(f.size)}</span></span>
      </div>`,
    )
    .join('');
}

/* ---------- incoming request ---------- */

function incomingSheet(ctx) {
  const incoming = ctx.s.incoming;
  const trusted = incoming.trust === 'trusted';
  const badge = trusted
    ? `Trusted device · ${fmtSize(incoming.total)} in total`
    : `New device · Check that the code ${incoming.verifyCode} also appears on ${incoming.peerName}`;
  return shell({
    labelledBy: 'incoming-title',
    title: `${escapeHtml(incoming.peerName)} wants to send you ${incoming.files.length} files`,
    body: `
      <p class="row support-line" style="color:var(--primary)">
        ${icon(trusted ? 'shield-check' : 'wifi', 24)}
        <span class="t-body-sm ink-secondary">${escapeHtml(badge)}</span>
      </p>
      <div class="list">${fileList(incoming.files)}</div>
      ${
        incoming.executable
          ? `<div class="banner">${icon('triangle-alert', 24)}<span class="banner-body">This file can install software. Only accept it if you know who sent it.</span></div>`
          : ''
      }
      <p class="t-body-sm ink-secondary">Saving to: ${escapeHtml(incoming.savingTo)}</p>
      ${
        trusted
          ? `<button type="button" class="check-row" role="checkbox" aria-checked="${ctx.ui.alwaysAccept ? 'true' : 'false'}"
              data-action="toggle-always-accept">
              <span class="check-box" aria-hidden="true">${icon('check', 16)}</span>
              <span class="check-label">Always accept files from this device</span>
            </button>`
          : ''
      }
    `,
    actions: `
      <button type="button" class="btn btn-primary btn-block" data-action="accept-incoming">
        Accept ${icon('check', 24)}
      </button>
      <button type="button" class="btn btn-quiet" data-action="decline-incoming">Decline</button>
    `,
  });
}

/* ---------- choose what to send ---------- */

function chooseSheet(ctx) {
  const { ui } = ctx;
  const rows = [
    ['photos', 'image', 'Photos and videos'],
    ['documents', 'file-text', 'Documents'],
    ['folders', 'folder', 'Folders'],
    ['clipboard', 'clipboard-paste', 'Text from the clipboard'],
    ['library', 'folder-open', 'Files already in DropGrab'],
  ];
  const total = ui.selection.reduce((sum, f) => sum + f.size, 0);
  return shell({
    labelledBy: 'choose-title',
    title: 'Choose what to send',
    body: `
      <p class="t-body ink-secondary">Pick files or folders, then pick a device</p>
      <div class="list">
        ${rows
          .map(
            ([key, iconName, label]) => `<button type="button" class="list-row" data-action="add-batch" data-batch="${key}">
              <span class="row-icon">${icon(iconName, 24)}</span>
              <span class="row-body"><span class="row-title">${label}</span></span>
              <span class="row-trailing">${icon('plus', 24)}</span>
            </button>`,
          )
          .join('')}
      </div>
      <p class="t-body-sm ink-muted" role="status">Selected: ${ui.selection.length} ${ui.selection.length === 1 ? 'file' : 'files'}${
        total ? ` · ${fmtSize(total)}` : ''
      }</p>
      ${
        ui.selection.length
          ? `<div class="selection-bar">
              <span class="selection-text">${ui.selection.length} ${ui.selection.length === 1 ? 'file' : 'files'} · ${fmtSize(total)}</span>
              <button type="button" class="btn btn-primary" data-action="send-selection">
                Send ${ui.selection.length} ${ui.selection.length === 1 ? 'file' : 'files'} ${icon('send', 16)}
              </button>
            </div>`
          : ''
      }
    `,
    actions: `<button type="button" class="btn btn-quiet" data-action="close-sheet">Cancel</button>`,
  });
}

/* ---------- visibility ---------- */

function visibilitySheet(ctx) {
  const options = [
    ['everyone', 'Everyone nearby', 'Anyone on this Wi-Fi network can ask to send files.'],
    ['trusted', 'Trusted devices only', 'Only devices you have confirmed can see this device.'],
    ['hidden', 'Hidden', 'No one can find this device. Pairing with a QR code still works.'],
  ];
  return shell({
    labelledBy: 'visibility-title',
    title: 'Who can see this device',
    body: `<div class="radio-list">
      ${options
        .map(
          ([value, label, detail]) => `<button type="button" class="radio-row" role="radio"
            aria-checked="${ctx.s.self.visibility === value ? 'true' : 'false'}"
            data-action="set-visibility" data-visibility="${value}">
            <span class="radio-dot" aria-hidden="true">${icon('check', 16)}</span>
            <span class="radio-body">
              <span class="radio-label">${label}</span>
              <span class="radio-detail">${detail}</span>
            </span>
          </button>`,
        )
        .join('')}
    </div>`,
    actions: `<button type="button" class="btn btn-quiet" data-action="close-sheet">Cancel</button>`,
  });
}

/* ---------- device detail, forget, block ---------- */

export function deviceSheet(ctx) {
  const device = ctx.s.devices.find((d) => d.id === ctx.ui.deviceId);
  if (!device) return '';
  return shell({
    labelledBy: 'device-title',
    title: escapeHtml(device.name),
    body: `
      <p class="row support-line" style="color:var(--primary)">
        ${icon(deviceIcon(device.type), 24)}
        <span class="t-body-sm ink-secondary">${escapeHtml(device.os)} · ${
          device.trust === 'trusted' ? 'trusted device' : 'not trusted yet'
        } · last seen ${escapeHtml(device.lastSeen)}</span>
      </p>
      <p class="fingerprint">Fingerprint ${escapeHtml(device.fingerprint)}</p>
      <div class="toggle-row">
        <span class="toggle-label">Accept files automatically</span>
        ${switchControl(`Accept files automatically from ${device.name}`, device.autoAccept, 'toggle-auto-accept')}
      </div>
      <div class="toggle-row">
        <span class="toggle-label">Allow downloads on this device</span>
        ${switchControl(`Allow downloads on ${device.name}`, device.allowRemote, 'toggle-allow-remote')}
      </div>
    `,
    actions: `
      <button type="button" class="btn btn-quiet" data-action="ask-forget">Forget this device</button>
      <button type="button" class="btn btn-quiet" data-action="block-device">Block this device</button>
    `,
  });
}

function forgetSheet(ctx) {
  const device = ctx.s.devices.find((d) => d.id === ctx.ui.deviceId);
  if (!device) return '';
  return shell({
    labelledBy: 'forget-title',
    title: `Forget ${escapeHtml(device.name)}?`,
    body: `<p class="t-body ink-secondary">You'll need to confirm a code the next time you connect.</p>`,
    actions: `
      <button type="button" class="btn btn-primary" data-action="confirm-forget">Forget</button>
      <button type="button" class="btn btn-quiet" data-action="close-sheet">Cancel</button>
    `,
  });
}

/* ---------- browser: detected items ---------- */

function detectedSheet(ctx) {
  const items = ctx.s.detected || [];
  return shell({
    labelledBy: 'detected-title',
    title: `Download · ${items.length} found`,
    body: `<div class="list">
      ${items
        .map(
          (item) => `<button type="button" class="list-row" data-action="pick-detected" data-item-id="${item.id}">
            <span class="row-icon">${icon(fileIcon(item.kind), 24)}</span>
            <span class="row-body">
              <span class="row-title">${escapeHtml(item.name)}</span>
              <span class="row-meta">${escapeHtml(item.duration)} · ${escapeHtml(item.quality)} · ${fmtSize(item.size)}</span>
            </span>
            <span class="row-trailing">${icon('download', 24)}</span>
          </button>`,
        )
        .join('')}
    </div>`,
    actions: `<button type="button" class="btn btn-quiet" data-action="close-sheet">Cancel</button>`,
  });
}

/* ---------- library: delete ---------- */

function deleteSheet(ctx) {
  const ids = ctx.ui.deleteIds || [];
  const items = ctx.s.library.filter((i) => ids.includes(i.id));
  const total = items.reduce((sum, i) => sum + i.size, 0);
  const single = items.length === 1;
  const title = single
    ? `Delete ${escapeHtml(items[0].name)}?`
    : `Delete ${items.length} files?`;
  const body = single
    ? 'This removes the file from this device.'
    : `This removes ${items.length} files (${fmtSize(total)}) from this device.`;
  return shell({
    labelledBy: 'delete-title',
    title,
    body: `<p class="t-body ink-secondary">${body}</p>`,
    actions: `
      <button type="button" class="btn btn-primary" data-action="confirm-delete">Delete</button>
      <button type="button" class="btn btn-quiet" data-action="close-sheet">Cancel</button>
    `,
  });
}

/* ---------- downloads: overflow menu ---------- */

function downloadMenuSheet(ctx) {
  const job = ctx.s.downloads.find((j) => j.id === ctx.ui.downloadId);
  if (!job) return '';
  const remote = ctx.s.devices.find((d) => d.trust === 'trusted' && d.allowRemote);
  return shell({
    labelledBy: 'download-menu-title',
    title: escapeHtml(job.name),
    body: `<div class="list">
      <button type="button" class="list-row" data-action="send-finished" data-id="${job.id}">
        <span class="row-icon">${icon('send', 24)}</span>
        <span class="row-body"><span class="row-title">Send to a device</span></span>
      </button>
      ${
        remote
          ? `<button type="button" class="list-row" data-action="remote-download" data-id="${job.id}">
              <span class="row-icon">${icon('monitor', 24)}</span>
              <span class="row-body">
                <span class="row-title">Download on ${escapeHtml(remote.name)}</span>
                <span class="row-meta">Only trusted devices that allow it</span>
              </span>
            </button>`
          : ''
      }
      <button type="button" class="list-row" data-action="remove-download" data-id="${job.id}">
        <span class="row-icon">${icon('trash-2', 24)}</span>
        <span class="row-body"><span class="row-title">Remove from list</span></span>
      </button>
    </div>`,
    actions: `<button type="button" class="btn btn-quiet" data-action="close-sheet">Cancel</button>`,
  });
}

/* ---------- settings: device name ---------- */

function deviceNameSheet(ctx) {
  const value = ctx.ui.deviceNameDraft ?? ctx.s.settings.deviceName;
  return shell({
    labelledBy: 'device-name-title',
    title: 'Device name',
    body: `<div class="field">
      <label class="field-label" for="device-name-input">Name other devices see</label>
      <input class="input" id="device-name-input" data-model="deviceName" data-focus-id="device-name"
        value="${escapeHtml(value)}" autocomplete="off" maxlength="40">
      <span class="field-hint">Used in nearby lists and transfer requests.</span>
    </div>`,
    actions: `
      <button type="button" class="btn btn-primary" data-action="save-device-name">Save</button>
      <button type="button" class="btn btn-quiet" data-action="close-sheet">Cancel</button>
    `,
  });
}

export function renderSheet(ctx) {
  if (ctx.s.incoming) return incomingSheet(ctx);
  switch (ctx.ui.sheet) {
    case 'choose':
      return chooseSheet(ctx);
    case 'visibility':
      return visibilitySheet(ctx);
    case 'device':
      return deviceSheet(ctx);
    case 'forget':
      return forgetSheet(ctx);
    case 'detected':
      return detectedSheet(ctx);
    case 'delete':
      return deleteSheet(ctx);
    case 'download-menu':
      return downloadMenuSheet(ctx);
    case 'device-name':
      return deviceNameSheet(ctx);
    default:
      return '';
  }
}

/* ---------- sheet actions ---------- */

export const actions = {
  scrim: (ctx, el, event) => {
    if (event.target === el) ctx.closeSheet();
  },

  'close-sheet': (ctx) => ctx.closeSheet(),

  'add-batch': (ctx, el) => {
    const batch = SEND_BATCHES[el.dataset.batch];
    if (!batch) return;
    if (batch.text) {
      const pending = ctx.app.ui.pendingDeviceId;
      if (!pending) {
        ctx.app.ui.pendingText = batch.text;
        ctx.closeSheet();
        ctx.go('nearby');
        ctx.toast('Choose a device to send the text to');
        return;
      }
      const device = ctx.s.devices.find((d) => d.id === pending);
      ctx.invoke('text.send', { peer: pending, text: batch.text });
      ctx.toast(device ? `Sent the text to ${device.name}` : 'Sent the text from the clipboard');
      ctx.closeSheet();
      return;
    }
    const existing = new Set(ctx.ui.selection.map((f) => f.id));
    ctx.app.ui.pendingText = null;
    for (const file of batch.files) {
      if (!existing.has(file.id)) ctx.ui.selection.push(file);
    }
    ctx.render();
  },

  'send-selection': async (ctx) => {
    if (!ctx.ui.selection.length) return;
    const pending = ctx.app.ui.pendingDeviceId;
    ctx.closeSheet();
    if (pending) {
      const { startSend } = await import('../screens/nearby.js');
      await startSend(ctx, pending);
      return;
    }
    ctx.app.ui.pickTarget = true;
    ctx.go('nearby');
    ctx.toast('Choose a device to send to');
  },

  'toggle-always-accept': (ctx) => {
    ctx.app.ui.alwaysAccept = !ctx.app.ui.alwaysAccept;
    ctx.render();
  },

  'accept-incoming': async (ctx) => {
    if (!ctx.s.incoming) return;
    await ctx.invoke('session.respond', {
      sid: ctx.s.incoming.sid,
      accept: true,
      trust: ctx.ui.alwaysAccept,
    });
    ctx.app.ui.alwaysAccept = false;
    ctx.go('transfer');
  },

  'decline-incoming': async (ctx) => {
    if (!ctx.s.incoming) return;
    await ctx.invoke('session.respond', { sid: ctx.s.incoming.sid, accept: false, trust: false });
    ctx.toast('Declined the files');
    ctx.render();
  },

  'set-visibility': async (ctx, el) => {
    const visibility = el.dataset.visibility;
    await ctx.invoke('visibility.set', { visibility });
    ctx.closeSheet();
  },

  'toggle-auto-accept': async (ctx) => {
    const device = ctx.s.devices.find((d) => d.id === ctx.ui.deviceId);
    if (!device) return;
    await ctx.invoke('devices.set_allowance', { deviceId: device.id, autoAccept: !device.autoAccept });
    ctx.render();
  },

  'toggle-allow-remote': async (ctx) => {
    const device = ctx.s.devices.find((d) => d.id === ctx.ui.deviceId);
    if (!device) return;
    await ctx.invoke('devices.set_allowance', { deviceId: device.id, allowRemote: !device.allowRemote });
    ctx.render();
  },

  'ask-forget': (ctx) => {
    ctx.app.ui.sheet = 'forget';
    ctx.render();
  },

  'confirm-forget': async (ctx) => {
    const device = ctx.s.devices.find((d) => d.id === ctx.ui.deviceId);
    if (device) await ctx.invoke('devices.forget', { deviceId: device.id });
    ctx.closeSheet();
    ctx.toast(device ? `Forgot ${device.name}` : 'Device forgotten');
  },

  'block-device': async (ctx) => {
    const device = ctx.s.devices.find((d) => d.id === ctx.ui.deviceId);
    if (device) await ctx.invoke('devices.block', { deviceId: device.id });
    ctx.closeSheet();
    ctx.toast(device ? `Blocked ${device.name}` : 'Device blocked');
  },

  'pick-detected': async (ctx, el) => {
    const item = (ctx.s.detected || []).find((i) => i.id === el.dataset.itemId);
    if (!item) return;
    const pageUrl = `https://${ctx.app.ui.browserUrl || 'example.org/lectures'}`;
    ctx.closeSheet();
    if (item.kind !== 'video') {
      const name = /\.[a-z0-9]{2,4}$/i.test(item.name) ? item.name : `${item.name}.pdf`;
      await ctx.invoke('downloads.add', { url: pageUrl, name, kind: 'direct', options: {} });
      ctx.go('downloads');
      ctx.toast(`Downloading ${name}`);
      return;
    }
    ctx.app.ui.qualityItem = { ...item, url: pageUrl };
    ctx.go('quality');
  },

  'remote-download': (ctx) => {
    const device = ctx.s.devices.find((d) => d.trust === 'trusted' && d.allowRemote);
    ctx.invoke('remote.request_download', { deviceId: device ? device.id : null });
    ctx.closeSheet();
    ctx.toast(device ? `${device.name} will download this file` : 'Remote download queued');
  },

  'save-device-name': async (ctx) => {
    const value = (ctx.ui.deviceNameDraft ?? ctx.s.settings.deviceName).trim();
    if (!value) {
      ctx.toast('Enter a name for this device');
      return;
    }
    await ctx.invoke('settings.set', { deviceName: value });
    ctx.app.ui.deviceNameDraft = null;
    ctx.closeSheet();
    ctx.toast('Device name saved');
  },

  'confirm-delete': async (ctx) => {
    const ids = ctx.ui.deleteIds || [];
    if (ids.length) await ctx.invoke('library.delete', { ids });
    ctx.app.ui.deleteIds = [];
    ctx.closeSheet();
    ctx.toast(ids.length === 1 ? 'Deleted 1 file' : `Deleted ${ids.length} files`);
  },
};
