// First run.

import { icon } from '../icons.js';
import { escapeHtml } from '../ui/dom.js';

export function render(ctx) {
  return `
    <div class="topbar">
      <span class="topbar-title">${escapeHtml(ctx.s.self.name)}</span>
    </div>
    <div class="first-run">
      <h1 class="display-copy">Send files to any nearby device</h1>
      <p class="t-body ink-secondary">Works between Android, iPhone, Windows, Mac, and Linux. No account. No internet needed.</p>
      <div class="explain-row">
        ${icon('radar', 24, 'pulse')}
        <p class="t-body">Looks for devices on your Wi-Fi network and by Bluetooth. Nothing leaves your devices.</p>
      </div>
      <button type="button" class="btn btn-primary btn-block" data-action="finish-first-run">
        Find nearby devices ${icon('arrow-right', 24)}
      </button>
      <div>
        <button type="button" class="btn btn-quiet" data-action="skip-first-run">Skip for now</button>
      </div>
    </div>
  `;
}

export const actions = {
  'finish-first-run': (ctx) => {
    try {
      localStorage.setItem('dropgrab.firstRun.v1', 'done');
    } catch {
      /* ignore */
    }
    ctx.go('nearby');
    ctx.toast('Looking for devices on this Wi-Fi network');
  },

  'skip-first-run': (ctx) => {
    try {
      localStorage.setItem('dropgrab.firstRun.v1', 'done');
    } catch {
      /* ignore */
    }
    ctx.go('nearby');
  },
};
