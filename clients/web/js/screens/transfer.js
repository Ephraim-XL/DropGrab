// Transfer in progress, paused, finished.

import { icon } from '../icons.js';
import { escapeHtml, fileIcon, fmtEta, fmtSize, fmtSpeed, pct, progressBar, truncateName } from '../ui/dom.js';

function statusLine(session) {
  if (session.state === 'transferring') {
    return `${fmtSize(session.done)} of ${fmtSize(session.total)} · ${fmtSpeed(session.bps)} · ${fmtEta(session.eta_s)}`;
  }
  if (session.state === 'paused') return `Paused at ${pct(session.done, session.total)}%`;
  if (session.state === 'offered') return 'Waiting for the other device to accept';
  if (session.state === 'done') return `${fmtSize(session.total)} in 21 s`;
  return '';
}

function filesWithProgress(session) {
  let budget = session.done;
  const rows = session.files.map((file) => {
    const done = Math.min(file.size, Math.max(0, budget));
    budget -= done;
    return { ...file, done, state: done >= file.size ? 'done' : 'pending' };
  });
  const active = rows.find((row) => row.state === 'pending');
  if (active) active.state = session.state === 'paused' ? 'paused' : 'sending';
  return rows;
}

export function render(ctx) {
  const { s } = ctx;
  const session = s.session;

  if (!session) {
    return `<div class="topbar">
        <button type="button" class="icon-btn" data-action="go-nearby" aria-label="Back to Nearby">${icon('chevron-left', 24)}</button>
        <span class="topbar-title">Transfer</span>
      </div>
      <div class="page-head"><h1 class="t-headline">No transfer in progress</h1></div>
      <p class="t-body ink-secondary prose">Start a send from the Nearby screen, or download a file and send it from Downloads.</p>
      <div><button type="button" class="btn btn-primary" data-action="go-nearby">Back to Nearby</button></div>`;
  }

  const verb = session.direction === 'in' ? 'Receiving' : 'Sending';
  const pastVerb = session.direction === 'in' ? 'Received' : 'Sent';
  const headline =
    session.state === 'done'
      ? session.message
      : session.state === 'cancelled'
        ? 'Transfer cancelled'
        : `${verb} ${session.files.length} files ${session.direction === 'in' ? 'from' : 'to'} ${escapeHtml(session.peerName)}`;

  const summary =
    session.state === 'done'
      ? `${fmtSize(session.total)} · ${session.files.length} files`
      : statusLine(session);

  const rows = filesWithProgress(session)
    .map((file) => {
      const iconName =
        file.state === 'done'
          ? 'check'
          : file.state === 'paused'
            ? 'pause'
            : file.state === 'sending'
              ? 'loader'
              : fileIcon(file.kind);
      const detail =
        file.state === 'done'
          ? fmtSize(file.size)
          : file.state === 'pending'
            ? 'Waiting'
            : `${fmtSize(file.done)} of ${fmtSize(file.size)}`;
      return `<div class="list-row">
        <span class="row-icon ${file.state === 'done' ? 'ink-primary' : file.state === 'sending' ? 'ink-accent' : ''}">
          ${icon(iconName, 24, file.state === 'sending' ? 'spin' : '')}
        </span>
        <span class="row-body">
          <span class="row-title" title="${escapeHtml(file.name)}">${escapeHtml(truncateName(file.name))}</span>
        </span>
        <span class="row-trailing"><span class="t-body-sm ink-muted">${detail}</span></span>
      </div>`;
    })
    .join('');

  const actions =
    session.state === 'done'
      ? `<button type="button" class="btn btn-primary" data-action="go-nearby">Done</button>
         <button type="button" class="btn btn-secondary" data-action="send-more">Send more</button>`
      : session.state === 'cancelled'
        ? `<button type="button" class="btn btn-primary" data-action="go-nearby">Back to Nearby</button>`
        : `<button type="button" class="btn btn-secondary" data-action="${session.state === 'paused' ? 'resume-transfer' : 'pause-transfer'}">
             ${icon(session.state === 'paused' ? 'play' : 'pause', 16)} ${session.state === 'paused' ? 'Resume' : 'Pause'}
           </button>
           <button type="button" class="btn btn-quiet" data-action="cancel-transfer">Cancel</button>`;

  return `
    <div class="topbar">
      <button type="button" class="icon-btn" data-action="go-nearby" aria-label="Back to Nearby">${icon('chevron-left', 24)}</button>
      <span class="topbar-title">${escapeHtml(session.peerName)}</span>
    </div>
    <div class="page-head">
      <h1 class="t-headline">${headline}</h1>
      <p class="support-line"><span class="t-numeric">${summary}</span></p>
    </div>
    <div class="section">
      ${session.state !== 'done' && session.state !== 'cancelled' ? progressBar(session.done, session.total, 'Transfer progress') : ''}
    </div>
    <div class="list">${rows}</div>
    <div class="row-16" style="flex-wrap:wrap">${actions}</div>
  `;
}

export const actions = {
  'go-nearby': (ctx) => ctx.go('nearby'),

  'pause-transfer': async (ctx) => {
    await ctx.invoke('session.pause', {});
    ctx.render();
  },

  'resume-transfer': async (ctx) => {
    await ctx.invoke('session.resume', {});
    ctx.render();
  },

  'cancel-transfer': async (ctx) => {
    await ctx.invoke('session.cancel', {});
    ctx.go('nearby');
    ctx.toast('Transfer cancelled');
  },

  'send-more': (ctx) => {
    ctx.app.ui.selection = [];
    ctx.go('nearby');
    ctx.openSheet('choose');
  },
};
