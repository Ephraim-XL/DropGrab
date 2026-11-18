// Nearby: the home screen and the hero feature.

import { icon } from '../icons.js';
import { iconButton, escapeHtml, fmtSize } from '../ui/dom.js';
import { deviceTile, linkField, recentRow } from '../ui/components.js';

const VISIBILITY_COPY = {
  everyone: 'Visible to everyone nearby',
  trusted: 'Visible to trusted devices only',
  hidden: 'Hidden from nearby devices',
};

export function render(ctx) {
  const { s, ui } = ctx;
  const devices = s.devices.filter((d) => d.online && d.trust !== 'blocked');
  const choosing = ui.selection.length > 0;
  const total = ui.selection.reduce((sum, f) => sum + f.size, 0);

  const tiles = devices.length
    ? devices
        .map((d) =>
          deviceTile(d, {
            isTarget: false,
            selection: ui.selection,
          }),
        )
        .join('')
    : '';

  const emptyHero = `<div class="section">
      <span class="ink-secondary">${icon('radar', 32)}</span>
      <h2 class="t-title">No devices found on this Wi-Fi network.</h2>
      <p class="t-body ink-secondary prose">Check that both devices are on the same network and that DropGrab is open on the other one.</p>
      <div class="row-16" style="flex-wrap:wrap">
        <button type="button" class="btn btn-primary" data-action="open-qr">Scan a QR code</button>
        <button type="button" class="btn btn-secondary" data-action="refresh-devices">Try again</button>
      </div>
    </div>`;

  const selectionHint = choosing
    ? `<div class="banner">
        ${icon('check', 24)}
        <span class="banner-body">${ui.selection.length} ${ui.selection.length === 1 ? 'file' : 'files'} · ${fmtSize(total)} ready. Choose a device to send to.</span>
        <button type="button" class="btn-quiet btn" data-action="clear-selection">Clear</button>
      </div>`
    : '';

  const devicesColumn = `<div class="col">
      <div class="hero-devices" id="hero-devices">
        ${devices.length ? tiles : emptyHero}
      </div>
      ${selectionHint}
      ${
        devices.length
          ? `<button type="button" class="btn btn-primary btn-block" data-action="choose-files">
              Choose files
              ${icon('folder', 24)}
            </button>`
          : ''
      }
      <div>
        <button type="button" class="btn btn-quiet" data-action="open-qr" style="padding:0">
          ${icon('scan-line', 16)} Scan a QR code
        </button>
      </div>
      <div class="section" style="margin-top: var(--sp-md)">
        ${linkField({ value: ui.linkUrl, primary: 'Download' })}
      </div>
    </div>`;

  const recentColumn = `<div class="col">
      <div class="section">
        <h2 class="section-title">Recent</h2>
        <div class="list">
          ${s.recent.map(recentRow).join('')}
        </div>
      </div>
      <p class="t-body-sm ink-muted">Drop files on a device tile to send them. Drop a link anywhere to download it.</p>
    </div>`;

  return `
    <div class="topbar">
      <span class="topbar-title">${escapeHtml(s.self.name)}</span>
      ${iconButton('qr-code', 'Show this device\'s pairing code', 'open-qr')}
    </div>
    <div class="page-head">
      <button type="button" class="btn btn-quiet" data-action="open-visibility"
        style="min-height:48px;padding:0">${escapeHtml(VISIBILITY_COPY[s.self.visibility] || VISIBILITY_COPY.everyone)}</button>
      <h1 class="t-headline">Send files to any nearby device</h1>
      <p class="support-line">
        ${icon('radar', 24, 'pulse')}
        <span>${
          devices.length
            ? `${devices.length} ${devices.length === 1 ? 'device' : 'devices'} found on this Wi-Fi network`
            : 'Looking for devices on this Wi-Fi network'
        }</span>
      </p>
    </div>
    <div class="split">
      ${devicesColumn}
      ${recentColumn}
    </div>
  `;
}

export const actions = {
  'choose-files': (ctx) => {
    ctx.app.ui.pendingDeviceId = null;
    ctx.openSheet('choose');
  },

  'clear-selection': (ctx) => {
    ctx.app.ui.selection = [];
    ctx.app.ui.pickTarget = false;
    ctx.render();
  },

  'open-visibility': (ctx) => ctx.openSheet('visibility'),

  'refresh-devices': (ctx) => {
    ctx.toast('Looking for devices');
    ctx.invoke('visibility.set', { visibility: ctx.s.self.visibility });
  },

  'tile-send': async (ctx, el) => {
    const deviceId = el.dataset.deviceId;
    const device = ctx.s.devices.find((d) => d.id === deviceId);
    if (!device) return;
    if (device.online === false) {
      ctx.app.ui.sheet = 'device';
      ctx.app.ui.deviceId = deviceId;
      ctx.render();
      return;
    }
    if (ctx.ui.selection.length === 0 && ctx.ui.pendingText) {
      await ctx.invoke('text.send', { peer: deviceId, text: ctx.ui.pendingText });
      ctx.app.ui.pendingText = null;
      ctx.toast(`Sent the text to ${device.name}`);
      ctx.render();
      return;
    }
    if (ctx.ui.selection.length === 0) {
      ctx.app.ui.pendingDeviceId = deviceId;
      ctx.openSheet('choose');
      return;
    }
    await startSend(ctx, deviceId);
  },
};

export async function startSend(ctx, deviceId) {
  const items = ctx.ui.selection;
  if (!items.length) return;
  try {
    await ctx.invoke('session.send', { peer: deviceId, items });
    ctx.app.ui.selection = [];
    ctx.app.ui.pickTarget = false;
    ctx.go('transfer');
  } catch (error) {
    ctx.toast(
      error.message === 'NET_UNREACHABLE'
        ? 'That device isn\'t reachable. Both devices need to be on the same Wi-Fi network.'
        : 'The send could not start. Try again.',
    );
  }
}
