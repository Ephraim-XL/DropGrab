// Devices: trusted devices and their allowances.

import { icon } from '../icons.js';
import { deviceIcon, escapeHtml, switchControl } from '../ui/dom.js';

function deviceCard(device) {
  return `<div class="device-card">
    <button type="button" class="device-card-head" data-action="open-device" data-id="${device.id}">
      <span class="row-icon">${icon(deviceIcon(device.type), 24)}</span>
      <span class="row-body">
        <span class="t-title">${escapeHtml(device.name)}</span>
        <span class="t-body-sm ink-muted">Trusted · last seen ${escapeHtml(device.lastSeen)}</span>
      </span>
      <span class="row-trailing">
        <span style="color:var(--accent)" aria-hidden="true">${icon('shield-check', 24)}</span>
        ${icon('chevron-right', 24)}
      </span>
    </button>
    <div class="toggle-row">
      <span class="toggle-label">Accept files automatically</span>
      ${switchControl(
        `Accept files automatically from ${device.name}`,
        device.autoAccept,
        'device-auto-accept',
        `data-id="${device.id}"`,
      )}
    </div>
    <div class="toggle-row">
      <span class="toggle-label">Allow downloads on this device</span>
      ${switchControl(
        `Allow downloads on ${device.name}`,
        device.allowRemote,
        'device-allow-remote',
        `data-id="${device.id}"`,
      )}
    </div>
  </div>`;
}

export function render(ctx) {
  const { s } = ctx;
  const trusted = s.devices.filter((d) => d.trust === 'trusted');

  const empty = `<div class="section">
      <span class="ink-secondary">${icon('shield-check', 32)}</span>
      <h2 class="t-title">No trusted devices.</h2>
      <p class="t-body ink-secondary prose">After your first transfer, you can choose to trust the device.</p>
      <div><button type="button" class="btn btn-primary" data-action="devices-nearby">Find nearby devices</button></div>
    </div>`;

  return `
    <div class="page-head">
      <h1 class="t-headline">Manage the devices you trust</h1>
      <p class="support-line">${trusted.length} trusted ${trusted.length === 1 ? 'device' : 'devices'}</p>
    </div>
    ${trusted.length ? `<div class="list">${trusted.map(deviceCard).join('')}</div>` : empty}
  `;
}

export const actions = {
  'open-device': (ctx, el) => {
    ctx.app.ui.deviceId = el.dataset.id;
    ctx.openSheet('device');
  },

  'device-auto-accept': async (ctx, el) => {
    const device = ctx.s.devices.find((d) => d.id === el.dataset.id);
    if (!device) return;
    await ctx.invoke('devices.set_allowance', { deviceId: device.id, autoAccept: !device.autoAccept });
    ctx.render();
  },

  'device-allow-remote': async (ctx, el) => {
    const device = ctx.s.devices.find((d) => d.id === el.dataset.id);
    if (!device) return;
    await ctx.invoke('devices.set_allowance', { deviceId: device.id, allowRemote: !device.allowRemote });
    ctx.render();
  },

  'devices-nearby': (ctx) => ctx.go('nearby'),
};
