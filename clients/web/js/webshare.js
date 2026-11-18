// Web share page (the receiver opens this in any browser).
// Standalone: no navigation, no session, no scripts from other sites.

import { icon } from './icons.js';
import { escapeHtml, fileIcon, fmtClock, fmtSize, truncateName } from './ui/dom.js';
import { FILES } from './mock/data.js';

const FILES_SHARED = [FILES.thesis, FILES.figures, FILES.defense];
const TOTAL = FILES_SHARED.reduce((sum, file) => sum + file.size, 0);
const PIN = '504193';
const EXPIRES_AT = Date.now() + 552000;

const root = document.getElementById('share-root');
let code = '';
let error = '';
let unlocked = false;

function render() {
  root.innerHTML = unlocked ? unlockedView() : lockedView();
  const input = root.querySelector('#share-code');
  if (input && !unlocked) {
    input.focus({ preventScroll: true });
    input.setSelectionRange(input.value.length, input.value.length);
  }
}

function lockedView() {
  return `
    <div class="page-head">
      <h1 class="t-headline">Download files from Kofi's Pixel 8</h1>
      <p class="support-line">${FILES_SHARED.length} files · ${fmtSize(TOTAL)}</p>
    </div>
    <div class="banner">
      ${icon('triangle-alert', 24)}
      <span class="banner-body">Your browser may warn that this connection isn't trusted. Check that the
        <span class="fingerprint" style="display:inline">fingerprint 4F3A 91C2 7D08 B6E1</span> matches the one on
        Kofi's Pixel&nbsp;8, then continue.</span>
    </div>
    <form data-share-form class="section" novalidate>
      <div class="field">
        <label class="field-label" for="share-code">Enter the 6-digit code shown on Kofi's Pixel&nbsp;8</label>
        <input class="input share-code-input" id="share-code" name="code" inputmode="numeric"
          autocomplete="one-time-code" maxlength="6" value="${escapeHtml(code)}" aria-describedby="code-hint" />
        <span class="field-hint" id="code-hint">The code changes every time a share starts.</span>
        ${error ? `<span class="field-error">${icon('triangle-alert', 16)} ${escapeHtml(error)}</span>` : ''}
      </div>
      <button type="submit" class="btn btn-primary btn-block">Show files ${icon('arrow-right', 24)}</button>
    </form>
    <p class="t-numeric ink-muted">Expires in ${fmtClock(EXPIRES_AT - Date.now())}</p>
  `;
}

function unlockedView() {
  return `
    <div class="page-head">
      <h1 class="t-headline">Download files from Kofi's Pixel 8</h1>
      <p class="support-line">${FILES_SHARED.length} files · ${fmtSize(TOTAL)}</p>
    </div>
    <div class="list">
      ${FILES_SHARED.map(
        (file) => `<div class="download-file-row">
          <span class="row-icon">${icon(fileIcon(file.kind), 24)}</span>
          <span class="row-body" style="flex:1;min-width:0">
            <span class="t-body" title="${escapeHtml(file.name)}">${escapeHtml(truncateName(file.name))}</span>
            <span class="t-body-sm ink-muted">${fmtSize(file.size)}</span>
          </span>
          <button type="button" class="icon-btn is-quiet-action" data-file="${escapeHtml(file.name)}"
            aria-label="Download ${escapeHtml(file.name)}" title="Download">${icon('download', 24)}</button>
        </div>`,
      ).join('')}
    </div>
    <button type="button" class="btn btn-primary btn-block" data-zip>
      Download all as .zip (${fmtSize(TOTAL)}) ${icon('download', 24)}
    </button>
    <p class="t-numeric ink-muted">Expires in ${fmtClock(EXPIRES_AT - Date.now())}</p>
    <p class="t-body-sm ink-secondary">Files stay on Kofi's device until you download them.</p>
  `;
}

root.addEventListener('input', (event) => {
  if (event.target.id === 'share-code') {
    code = event.target.value.replace(/\D/g, '').slice(0, 6);
    event.target.value = code;
  }
});

root.addEventListener('submit', (event) => {
  event.preventDefault();
  if (unlocked) return;
  if (code === PIN) {
    error = '';
    unlocked = true;
    render();
  } else {
    error = 'That code doesn\'t match. Check the six digits on Kofi\'s Pixel 8.';
    render();
  }
});

root.addEventListener('click', (event) => {
  const target = event.target.closest('[data-file], [data-zip]');
  if (!target) return;
  const label = target.dataset.file || 'the .zip archive';
  const note = document.createElement('div');
  note.className = 'toast';
  note.setAttribute('role', 'status');
  note.textContent = `Downloading ${label}`;
  document.body.appendChild(note);
  setTimeout(() => note.remove(), 2400);
});

render();
setInterval(() => {
  if (unlocked) return;
  const expiry = root.querySelector('.t-numeric.ink-muted');
  if (expiry) expiry.textContent = `Expires in ${fmtClock(EXPIRES_AT - Date.now())}`;
}, 1000);
