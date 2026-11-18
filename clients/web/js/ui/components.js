// Shared screen components.

import { icon } from '../icons.js';
import {
  deviceIcon,
  deviceMeta,
  escapeHtml,
  fileIcon,
  fmtSize,
  progressBar,
  truncateName,
} from './dom.js';

export function deviceTile(device, { isTarget = false, selection = [], sending = null } = {}) {
  const meta = sending
    ? ''
    : device.online === false
      ? `Unreachable · ${device.link}`
      : `${deviceMeta(device)}`;
  const action = isTarget
    ? `${icon('check', 24)}<span class="sr-only">Send ${selection.length} files to ${escapeHtml(device.name)}</span>`
    : `${icon('send', 24)}<span class="sr-only">Send to ${escapeHtml(device.name)}</span>`;
  return `<button type="button" class="device-tile${isTarget ? ' is-target' : ''}"
      data-action="tile-send" data-device-id="${device.id}"
      aria-label="Send to ${escapeHtml(device.name)}">
    <span class="tile-icon">${icon(deviceIcon(device.type), 24)}</span>
    <span class="tile-body">
      <span class="tile-name">${escapeHtml(device.name)}</span>
      <span class="tile-meta">${
        sending
          ? `${fmtSize(sending.done)} of ${fmtSize(sending.total)}`
          : escapeHtml(meta)
      }</span>
      ${sending ? `<span class="tile-progress">${progressBar(sending.done, sending.total, `Sending to ${device.name}`)}</span>` : ''}
    </span>
    <span class="tile-action" aria-hidden="true">${action}</span>
  </button>`;
}

export function linkField({ value = '', primary = 'Download', paste = false, id = 'link' } = {}) {
  const input = `<input class="input" id="${id}-input" data-focus-id="${id}-input" data-model="${id}"
        type="url" value="${escapeHtml(value)}" aria-describedby="${id}-hint" autocomplete="off" spellcheck="false">`;
  if (paste) {
    return `<div class="field">
      <label class="field-label" for="${id}-input">Download from a link</label>
      <span class="field-hint" id="${id}-hint">Paste a link to a file, video, or song.</span>
      <div class="input-row">
        ${input}
        <button type="button" class="btn btn-secondary" data-action="paste-link">Paste</button>
      </div>
      <button type="button" class="btn btn-primary btn-block" data-action="start-link-download">
        Download ${icon('download', 16)}
      </button>
    </div>`;
  }
  return `<div class="field">
    <label class="field-label" for="${id}-input">Download from a link</label>
    <span class="field-hint" id="${id}-hint">Paste a link to a file, video, or song.</span>
    <div class="input-row">
      ${input}
      <button type="button" class="btn btn-secondary" data-action="start-link-download">${primary}</button>
    </div>
  </div>`;
}

export function recentRow(item) {
  const direction = item.direction === 'sent' ? 'Sent to' : 'Received from';
  return `<div class="list-row">
    <span class="row-icon">${icon(fileIcon(item.kind), 24)}</span>
    <span class="row-body">
      <span class="row-title" title="${escapeHtml(item.name)}">${escapeHtml(truncateName(item.name))}</span>
      <span class="row-meta">${direction} ${escapeHtml(item.peer)} · ${fmtSize(item.size)} · ${escapeHtml(item.when)}</span>
    </span>
  </div>`;
}

export function fileRow(file, { trailing = '', state = null } = {}) {
  const stateIcon =
    state === 'done'
      ? `<span class="row-icon ink-primary">${icon('check', 24)}</span>`
      : state === 'sending'
        ? `<span class="row-icon ink-accent">${icon('loader', 24, 'spin')}</span>`
        : `<span class="row-icon">${icon(fileIcon(file.kind), 24)}</span>`;
  return `<div class="list-row">
    ${stateIcon}
    <span class="row-body">
      <span class="row-title" title="${escapeHtml(file.name)}">${escapeHtml(truncateName(file.name))}</span>
      <span class="row-meta">${escapeHtml(file.detail || fmtSize(file.size))}</span>
    </span>
    ${trailing ? `<span class="row-trailing">${trailing}</span>` : ''}
  </div>`;
}

export function emptyState({ iconName = 'radar', title, body, actions = '' }) {
  return `<div class="section">
    <span class="ink-secondary">${icon(iconName, 32)}</span>
    <h2 class="t-title">${escapeHtml(title)}</h2>
    <p class="t-body ink-secondary prose">${escapeHtml(body)}</p>
    ${actions ? `<div class="row-16" style="flex-wrap:wrap">${actions}</div>` : ''}
  </div>`;
}
