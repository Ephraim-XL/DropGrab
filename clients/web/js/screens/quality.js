// Choose a quality (format picker).

import { icon } from '../icons.js';
import { escapeHtml, fmtSize } from '../ui/dom.js';
import { QUALITY_OPTIONS } from '../mock/data.js';

function currentItem(ui) {
  return (
    ui.qualityItem || {
      title: 'Rust ownership explained',
      name: 'Rust ownership explained.mp4',
      duration: '18:42',
      kind: 'video',
      url: 'https://video.example.org/watch/rust-ownership',
      formats: QUALITY_OPTIONS,
    }
  );
}

// Detected items carry a title without an extension; downloads need a file name.
function fileName(item) {
  const base = item.name || item.title || 'download';
  return /\.[a-z0-9]{2,4}$/i.test(base) ? base : `${base}.mp4`;
}

export function render(ctx) {
  const { ui } = ctx;
  const item = currentItem(ui);
  const formats = item.formats?.length ? item.formats : QUALITY_OPTIONS;
  const selectedId = ui.quality || 'balanced';
  const selected = formats.find((f) => f.id === selectedId) || formats[1];

  return `
    <div class="topbar">
      <button type="button" class="icon-btn" data-action="quality-back" aria-label="Back">${icon('chevron-left', 24)}</button>
      <span class="topbar-title">Download options</span>
    </div>
    <div class="page-head">
      <h1 class="t-headline">Choose a quality to download</h1>
      <p class="support-line">${escapeHtml(item.title)} · ${escapeHtml(item.duration || '')}</p>
    </div>
    <div class="section">
      <div class="quality-chips chip-row">
        ${formats
          .map(
            (f) => `<button type="button" class="chip${f.id === selectedId ? ' is-selected' : ''}"
              data-action="select-quality" data-quality="${f.id}" aria-pressed="${f.id === selectedId}">
              ${f.id === selectedId ? icon('check', 16) : ''}${escapeHtml(f.label)}
            </button>`,
          )
          .join('')}
      </div>
      <div class="radio-list quality-list">
        ${formats
          .map(
            (f) => `<button type="button" class="radio-row" role="radio" aria-checked="${f.id === selectedId}"
              data-action="select-quality" data-quality="${f.id}">
              <span class="radio-dot" aria-hidden="true">${icon('check', 16)}</span>
              <span class="radio-body">
                <span class="radio-label">${escapeHtml(f.label)}</span>
                <span class="radio-detail">${escapeHtml(f.detail)}</span>
              </span>
              <span class="radio-size">${fmtSize(f.size)}</span>
            </button>`,
          )
          .join('')}
      </div>
      <p class="t-body-sm ink-secondary">Save to: Videos</p>
    </div>
    <div>
      <button type="button" class="btn btn-primary btn-block" data-action="quality-download">
        Download ${fmtSize(selected.size)} ${icon('download', 24)}
      </button>
    </div>
  `;
}

export const actions = {
  'select-quality': (ctx, el) => {
    ctx.app.ui.quality = el.dataset.quality;
    ctx.render();
  },

  'quality-back': (ctx) => {
    ctx.app.ui.sheet = null;
    ctx.go('downloads');
  },

  'quality-download': async (ctx) => {
    const item = currentItem(ctx.ui);
    const quality = ctx.ui.quality || 'balanced';
    await ctx.invoke('downloads.add', {
      url: item.url || 'https://video.example.org/watch/rust-ownership',
      name: fileName(item),
      kind: 'media',
      options: { quality },
    });
    ctx.go('downloads');
    ctx.toast('Download started');
  },
};
