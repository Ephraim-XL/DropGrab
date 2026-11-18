// Browser: address bar, page view, detected media.

import { icon } from '../icons.js';
import { escapeHtml } from '../ui/dom.js';

export function render(ctx) {
  const { s, ui } = ctx;
  const url = ui.browserUrl ?? 'example.org/lectures';
  const detected = s.detected || [];
  const privateMode = !!s.settings.privateMode;

  return `
    <div class="page-head">
      <div class="headline-row">
        <h1 class="t-headline">Find videos and files on any web page</h1>
        ${
          privateMode
            ? `<span title="Private mode is on" aria-label="Private mode is on"
                style="color:var(--accent);padding-top:var(--sp-xs)">${icon('shield-check', 24)}</span>`
            : ''
        }
      </div>
      <p class="support-line">Open a page and tap Download when it appears</p>
    </div>
    <form data-form="browser" class="row" style="gap:var(--sp-sm)" novalidate>
      <input class="input" type="text" inputmode="url" data-model="browserUrl" data-focus-id="browser-url"
        aria-label="Address" value="${escapeHtml(url)}" autocomplete="off" spellcheck="false" style="flex:1">
      <button type="submit" class="icon-btn" aria-label="Open the address" title="Open">${icon('arrow-right', 24)}</button>
      <button type="button" class="icon-btn" data-action="browser-clear" aria-label="Clear the address" title="Clear">${icon('x', 24)}</button>
    </form>
    <div class="browser-page">
      <p class="page-kicker">${escapeHtml(url.includes('/') ? url : `${url}/`)}</p>
      <h2 class="page-title">Lecture 7: Operating systems</h2>
      <p class="t-body ink-secondary prose">Course notes, slides, and the recorded lecture for week 7. Recorded on 4 March, 1 h 12 min.</p>
      <p class="t-body-sm ink-muted">${detected.length} items on this page can be downloaded</p>
    </div>
    ${
      detected.length
        ? `<div class="floating-action">
            <button type="button" class="btn btn-primary btn-block" data-action="open-detected">
              Download · ${detected.length} found ${icon('download', 24)}
            </button>
          </div>`
        : ''
    }
  `;
}

export const actions = {
  'browser-clear': (ctx) => {
    ctx.app.ui.browserUrl = '';
    ctx.render();
  },

  'browser-go': async (ctx) => {
    const url = (ctx.app.ui.browserUrl || '').trim() || 'example.org/lectures';
    ctx.app.ui.browserUrl = url;
    const result = await ctx.invoke('extractor.resolve', { url });
    ctx.s.detected = [
      {
        id: 'det-lecture-stream',
        name: 'Lecture 07 Operating Systems',
        kind: 'video',
        duration: result.duration,
        quality: '1080p',
        size: 734e6,
        formats: result.formats,
      },
      {
        id: 'det-slides',
        name: 'Chapter 4 slides',
        kind: 'document',
        duration: 'PDF',
        quality: 'Original',
        size: 12.4e6,
        formats: [],
      },
    ];
    ctx.app.ui.detectedCount = 2;
    ctx.render();
  },

  'open-detected': (ctx) => ctx.openSheet('detected'),
};
