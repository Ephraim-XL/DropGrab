// Small rendering helpers. No framework.

import { icon } from '../icons.js';

export function escapeHtml(value) {
  return String(value ?? '').replace(/[&<>"']/g, (ch) => ({
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#39;',
  }[ch]));
}

const MB = 1e6;
const GB = 1e9;

const trimZero = (text) => text.replace(/\.0$/, '');

export function fmtSize(bytes) {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 MB';
  if (bytes >= GB) {
    const gb = bytes / GB;
    return `${gb < 100 ? trimZero(gb.toFixed(1)) : Math.round(gb)} GB`;
  }
  const mb = bytes / MB;
  if (mb < 1) return `${Math.max(1, Math.round(bytes / 1e3))} KB`;
  return `${mb < 100 ? trimZero(mb.toFixed(1)) : Math.round(mb)} MB`;
}

export function fmtSpeed(bps) {
  if (!bps) return '';
  return `${(bps / MB).toFixed(1)} MB/s`;
}

export function fmtEta(seconds) {
  if (seconds == null) return '';
  if (seconds < 60) return `${Math.max(1, Math.round(seconds))} s left`;
  return `${Math.round(seconds / 60)} min left`;
}

export function fmtClock(ms) {
  const total = Math.max(0, Math.round(ms / 1000));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${String(s).padStart(2, '0')}`;
}

export function pct(done, total) {
  if (!total) return 0;
  return Math.max(0, Math.min(100, Math.round((done / total) * 100)));
}

// File names truncate in the middle so the extension stays visible.
export function truncateName(name, max = 52) {
  if (name.length <= max) return name;
  const dot = name.lastIndexOf('.');
  const ext = dot > 0 && name.length - dot <= 8 ? name.slice(dot) : '';
  const base = ext ? name.slice(0, dot) : name;
  const keep = Math.max(6, max - ext.length - 1);
  const head = Math.ceil(keep * 0.6);
  const tail = Math.max(0, keep - head);
  return `${base.slice(0, head)}…${base.slice(base.length - tail)}${ext}`;
}

const FILE_ICONS = {
  video: 'file-video',
  audio: 'file-audio',
  image: 'image',
  document: 'file-text',
  archive: 'file-archive',
  app: 'package',
};

export function fileIcon(kind) {
  return FILE_ICONS[kind] || 'file-text';
}

const DEVICE_ICONS = {
  phone: 'smartphone',
  tablet: 'tablet',
  laptop: 'laptop',
  desktop: 'monitor',
  web: 'globe',
};

export function deviceIcon(type) {
  return DEVICE_ICONS[type] || 'monitor';
}

export function fileNameCell(item) {
  return `<span class="name" title="${escapeHtml(item.name)}">${escapeHtml(truncateName(item.name))}</span>`;
}

export function iconButton(name, label, action, extra = '') {
  return `<button type="button" class="icon-btn" data-action="${action}" aria-label="${escapeHtml(label)}"
    title="${escapeHtml(label)}" ${extra}>${icon(name, 24)}</button>`;
}

export function progressBar(done, total, label) {
  const value = pct(done, total);
  return `<div class="progress" role="progressbar" aria-valuemin="0" aria-valuemax="100"
      aria-valuenow="${value}" aria-label="${escapeHtml(label)}">
    <div class="progress-track"><div class="progress-fill" style="width:${value}%"></div></div>
  </div>`;
}

export function switchControl(label, checked, action, attrs = '') {
  return `<button type="button" class="switch" role="switch" aria-checked="${checked ? 'true' : 'false'}"
      aria-label="${escapeHtml(label)}" data-action="${action}" ${attrs}>
    <span class="switch-track" aria-hidden="true"><span class="switch-knob"></span></span>
    <span class="switch-state">${checked ? 'On' : 'Off'}</span>
  </button>`;
}

export function deviceMeta(device) {
  const parts = [device.os, device.link];
  if (device.trust === 'trusted') parts.push('trusted');
  if (device.trust === 'blocked') parts.push('blocked');
  return parts.filter(Boolean).join(' · ');
}
