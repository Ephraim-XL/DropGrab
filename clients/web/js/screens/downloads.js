// Downloads: link intake, active jobs, paused jobs, finished jobs.

import { icon } from '../icons.js';
import { escapeHtml, fileIcon, fmtEta, fmtSize, fmtSpeed, pct, progressBar, truncateName } from '../ui/dom.js';
import { linkField } from '../ui/components.js';

function jobCard(job) {
  const percent = pct(job.done, job.total);
  const downloading = job.state === 'downloading';
  const probing = job.state === 'probing';
  const eta = downloading ? fmtEta(job.bps ? Math.round((job.total - job.done) / job.bps) : null) : '';
  const meta = probing
    ? 'Checking the link'
    : downloading
      ? `${escapeHtml(job.quality)} · ${fmtSize(job.total)} · ${fmtSpeed(job.bps)} · ${eta}`
      : `Paused at ${percent}% · ${fmtSize(job.done)} of ${fmtSize(job.total)}`;
  return `<div class="download-card">
    <div class="download-head">
      <span class="row-icon">${icon(fileIcon(job.kind), 24)}</span>
      <span class="row-body">
        <span class="row-title" title="${escapeHtml(job.name)}">${escapeHtml(truncateName(job.name))}</span>
        <span class="row-meta">${meta}</span>
      </span>
      <span class="download-row-actions">
        ${
          probing
            ? ''
            : `<button type="button" class="icon-btn is-quiet-action" data-action="${downloading ? 'download-pause' : 'download-resume'}"
          data-id="${job.id}" aria-label="${downloading ? 'Pause' : 'Resume'} ${escapeHtml(job.name)}"
          title="${downloading ? 'Pause' : 'Resume'}">${icon(downloading ? 'pause' : 'play', 24)}</button>`
        }
      </span>
    </div>
    ${
      probing
        ? ''
        : `<div class="row" style="gap:var(--sp-md)">
      <div style="flex:1">${progressBar(job.done, job.total, `${job.name} download progress`)}</div>
      <span class="t-numeric ink-muted" aria-hidden="true">${percent}%</span>
    </div>`
    }
  </div>`;
}

function finishedRow(job) {
  return `<div class="list-row">
    <span class="row-icon">${icon(fileIcon(job.kind), 24)}</span>
    <span class="row-body">
      <span class="row-title" title="${escapeHtml(job.name)}">${escapeHtml(truncateName(job.name))}</span>
      <span class="row-meta">${fmtSize(job.total)} · today</span>
    </span>
    <span class="row-trailing">
      <button type="button" class="icon-btn is-quiet-action" data-action="send-finished" data-id="${job.id}"
        aria-label="Send ${escapeHtml(job.name)} to a device" title="Send to a device">${icon('send', 24)}</button>
      <button type="button" class="icon-btn" data-action="download-menu" data-id="${job.id}"
        aria-label="More actions for ${escapeHtml(job.name)}" title="More actions">${icon('ellipsis', 24)}</button>
    </span>
  </div>`;
}

export function render(ctx) {
  const { s, ui } = ctx;
  const downloading = s.downloads.filter((j) => j.state === 'downloading' || j.state === 'probing');
  const paused = s.downloads.filter((j) => j.state === 'paused');
  const finished = s.downloads.filter((j) => j.state === 'finished');

  const supportParts = [];
  if (downloading.length) supportParts.push(`${downloading.length} downloading`);
  if (paused.length) supportParts.push(`${paused.length} paused`);
  const support = supportParts.join(', ') || 'No downloads in progress';

  const empty = `<div class="section">
      <span class="ink-secondary">${icon('download', 32)}</span>
      <h2 class="t-title">No downloads yet</h2>
      <p class="t-body ink-secondary prose">Paste a link above or open the Browser tab to find videos.</p>
    </div>`;

  return `
    <div class="page-head">
      <h1 class="t-headline">Download files, videos, and music from a link</h1>
      <p class="support-line">${escapeHtml(support)}</p>
    </div>
    ${
      ui.clipboardBanner === false
        ? ''
        : `<div class="banner">
            ${icon('clipboard-paste', 24)}
            <span class="banner-body">
              Download the link you copied? example.org/files/report.pdf
              <span class="banner-actions">
                <button type="button" class="btn btn-quiet btn-sm" data-action="clipboard-download">Download</button>
                <button type="button" class="btn btn-quiet btn-sm" data-action="dismiss-clipboard">Dismiss</button>
              </span>
            </span>
          </div>`
    }
    ${linkField({ value: ui.linkUrl, paste: true })}
    ${
      s.downloads.length === 0
        ? empty
        : `
      ${
        downloading.length
          ? `<div class="section">
              <h2 class="section-title">Downloading</h2>
              ${downloading.map(jobCard).join('')}
            </div>`
          : ''
      }
      ${
        paused.length
          ? `<div class="section">
              <h2 class="section-title">Paused</h2>
              ${paused.map(jobCard).join('')}
            </div>`
          : ''
      }
      ${
        finished.length
          ? `<div class="section">
              <h2 class="section-title">Finished</h2>
              <div class="list">${finished.map(finishedRow).join('')}</div>
            </div>`
          : ''
      }
    `
    }
  `;
}

export const actions = {
  'paste-link': async (ctx) => {
    let text = '';
    try {
      text = await navigator.clipboard.readText();
    } catch {
      text = '';
    }
    ctx.app.ui.linkUrl = text || 'https://example.org/files/report.pdf';
    ctx.render();
    ctx.toast('Pasted the link from the clipboard');
  },

  'start-link-download': async (ctx) => {
    const typed = (ctx.app.ui.linkUrl || '').trim();
    if (!typed) {
      ctx.toast('Paste a link to a file, video, or song');
      return;
    }
    const url = typed;
    ctx.app.ui.linkUrl = url;
    const probe = await ctx.invoke('downloads.probe', { url });
    if (probe.kind === 'media') {
      ctx.app.ui.qualityItem = {
        name: `${probe.title}.mp4`,
        title: probe.title,
        duration: probe.duration,
        kind: 'video',
        url,
        size: probe.formats?.[1]?.size || 198e6,
        formats: probe.formats,
      };
      ctx.go('quality');
      return;
    }
    await ctx.invoke('downloads.add', { url, kind: 'direct', options: {} });
    ctx.toast(`Downloading ${probe.filename}`);
    ctx.render();
  },

  'clipboard-download': async (ctx) => {
    await ctx.invoke('downloads.add', {
      url: 'https://example.org/files/report.pdf',
      kind: 'direct',
      options: {},
    });
    ctx.app.ui.clipboardBanner = false;
    ctx.toast('Downloading report.pdf');
    ctx.render();
  },

  'dismiss-clipboard': (ctx) => {
    ctx.app.ui.clipboardBanner = false;
    ctx.render();
  },

  'download-pause': async (ctx, el) => {
    await ctx.invoke('downloads.pause', { id: el.dataset.id });
    ctx.render();
  },

  'download-resume': async (ctx, el) => {
    await ctx.invoke('downloads.resume', { id: el.dataset.id });
    ctx.render();
  },

  'download-menu': (ctx, el) => {
    ctx.app.ui.downloadId = el.dataset.id;
    ctx.openSheet('download-menu');
  },

  'send-finished': (ctx, el) => {
    const job = ctx.s.downloads.find((j) => j.id === el.dataset.id);
    if (!job) return;
    ctx.app.ui.selection = [{ id: job.id, name: job.name, kind: job.kind, size: job.total }];
    ctx.app.ui.pickTarget = true;
    ctx.go('nearby');
    ctx.toast('Choose a device to send to');
  },

  'remove-download': async (ctx) => {
    const id = ctx.app.ui.downloadId;
    if (id) await ctx.invoke('downloads.cancel', { id });
    ctx.closeSheet();
    ctx.toast('Removed from the list');
  },
};
