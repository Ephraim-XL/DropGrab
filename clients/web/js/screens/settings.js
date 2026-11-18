// Settings.

import { icon } from '../icons.js';
import { escapeHtml, switchControl } from '../ui/dom.js';

const VISIBILITY_LABEL = {
  everyone: 'Everyone nearby',
  trusted: 'Trusted devices only',
  hidden: 'Hidden',
};

const QUALITY_LABEL = {
  best: 'Best',
  balanced: 'Balanced',
  'data-saver': 'Data saver',
  audio: 'Audio only',
};

function valueRow(label, value, action) {
  return `<button type="button" class="settings-row" data-action="${action}">
    <span class="settings-body">
      <span class="settings-label">${label}</span>
      <span class="settings-value">${escapeHtml(value)}</span>
    </span>
    <span class="chevron" aria-hidden="true">${icon('chevron-right', 24)}</span>
  </button>`;
}

function selectRow(label, setting, value, options) {
  return `<div class="settings-row">
    <label class="settings-body" for="set-${setting}">
      <span class="settings-label">${label}</span>
    </label>
    <select class="input" id="set-${setting}" data-setting="${setting}" data-focus-id="set-${setting}"
      style="width:auto;min-width:128px">
      ${options
        .map(
          ([val, text]) =>
            `<option value="${val}"${String(val) === String(value) ? ' selected' : ''}>${escapeHtml(text)}</option>`,
        )
        .join('')}
    </select>
  </div>`;
}

function toggleRow(label, setting, checked) {
  return `<div class="settings-row">
    <span class="settings-body"><span class="settings-label">${label}</span></span>
    ${switchControl(label, checked, 'setting-toggle', `data-setting="${setting}"`)}
  </div>`;
}

export function render(ctx) {
  const { s } = ctx;
  const st = s.settings;

  return `
    <div class="page-head">
      <h1 class="t-headline">Settings</h1>
    </div>

    <section class="settings-group">
      <h2>This device</h2>
      <div class="settings-card">
        ${valueRow('Device name', st.deviceName, 'edit-device-name')}
        ${valueRow('Visibility', VISIBILITY_LABEL[st.visibility] || VISIBILITY_LABEL.everyone, 'settings-visibility')}
      </div>
    </section>

    <section class="settings-group">
      <h2>Receiving</h2>
      <div class="settings-card">
        ${valueRow('Save location', st.saveLocation, 'setting-toast')}
        ${toggleRow('Ask before accepting', 'askBeforeAccepting', st.askBeforeAccepting)}
      </div>
    </section>

    <section class="settings-group">
      <h2>Downloads</h2>
      <div class="settings-card">
        ${valueRow('Download folder', st.downloadFolder, 'setting-toast')}
        ${selectRow('Downloads at once', 'downloadsAtOnce', st.downloadsAtOnce, [
          ['1', '1'],
          ['2', '2'],
          ['3', '3'],
          ['4', '4'],
          ['5', '5'],
        ])}
        ${toggleRow('Wi-Fi only', 'wifiOnly', st.wifiOnly)}
        ${selectRow('Speed limit', 'speedLimit', st.speedLimit, [
          ['off', 'Off'],
          ['5', '5 MB/s'],
          ['10', '10 MB/s'],
        ])}
      </div>
    </section>

    <section class="settings-group">
      <h2>Media</h2>
      <div class="settings-card">
        ${selectRow('Default quality', 'defaultQuality', st.defaultQuality, [
          ['best', 'Best'],
          ['balanced', 'Balanced'],
          ['data-saver', 'Data saver'],
          ['audio', 'Audio only'],
        ])}
        ${selectRow('Audio format', 'audioFormat', st.audioFormat, [
          ['M4A', 'M4A'],
          ['MP3', 'MP3'],
          ['FLAC', 'FLAC'],
        ])}
      </div>
    </section>

    <section class="settings-group">
      <h2>Appearance</h2>
      <div class="settings-card">
        <div class="settings-row">
          <span class="settings-body">
            <span class="settings-label">Theme</span>
            <span class="settings-value">${
              st.theme === 'system' ? 'Follows the system' : st.theme === 'dark' ? 'Dark' : 'Light'
            }</span>
          </span>
          <div class="segmented" role="group" aria-label="Theme">
            ${['system', 'light', 'dark']
              .map(
                (mode) =>
                  `<button type="button" data-action="set-theme" data-theme="${mode}"
                    aria-pressed="${st.theme === mode}">${mode[0].toUpperCase() + mode.slice(1)}</button>`,
              )
              .join('')}
          </div>
        </div>
        <div class="settings-row">
          <span class="settings-body">
            <span class="settings-label">Text size</span>
          </span>
          <div class="segmented" role="group" aria-label="Text size">
            ${[
              ['default', 'Default'],
              ['large', 'Large'],
              ['largest', 'Largest'],
            ]
              .map(
                ([value, label]) =>
                  `<button type="button" data-action="set-text-size" data-size="${value}"
                    aria-pressed="${st.textSize === value}">${label}</button>`,
              )
              .join('')}
          </div>
        </div>
      </div>
    </section>

    <section class="settings-group">
      <h2>Privacy</h2>
      <div class="settings-card">
        ${valueRow('Clear history', 'Removes transfers and downloads from this device', 'clear-history')}
        ${valueRow('Clear saved logins', 'Sign-ins kept for downloads', 'clear-logins')}
      </div>
    </section>

    <section class="settings-group">
      <h2>About</h2>
      <div class="settings-card">
        ${valueRow('Version', st.version, 'setting-toast')}
        ${valueRow('Open source notices', 'Lucide icons, Inter typeface', 'open-notices')}
        ${valueRow('Contact', 'support@dropgrab.example', 'setting-toast')}
      </div>
    </section>
  `;
}

export const actions = {
  'edit-device-name': (ctx) => ctx.openSheet('device-name'),

  'settings-visibility': (ctx) => ctx.openSheet('visibility'),

  'setting-toggle': async (ctx, el) => {
    const key = el.dataset.setting;
    const next = !ctx.s.settings[key];
    await ctx.invoke('settings.set', { [key]: next });
    ctx.render();
  },

  'set-theme': async (ctx, el) => {
    await ctx.invoke('settings.set', { theme: el.dataset.theme });
    ctx.render();
  },

  'set-text-size': async (ctx, el) => {
    await ctx.invoke('settings.set', { textSize: el.dataset.size });
    ctx.render();
  },

  'setting-toast': (ctx) => ctx.toast('This row opens the system picker in the desktop app'),

  'clear-history': (ctx) => {
    ctx.s.recent = [];
    ctx.toast('Cleared transfer and download history');
    ctx.render();
  },

  'clear-logins': (ctx) => ctx.toast('Cleared saved logins'),

  'open-notices': (ctx) => ctx.toast('Lucide icons (ISC) and the Inter typeface (OFL)'),
};
