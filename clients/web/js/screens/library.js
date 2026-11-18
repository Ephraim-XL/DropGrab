// Library: search, filters, selection, delete.

import { icon } from '../icons.js';
import { escapeHtml, fileIcon, fmtSize, truncateName } from '../ui/dom.js';

const FILTERS = [
  ['all', 'All'],
  ['videos', 'Videos'],
  ['music', 'Music'],
  ['images', 'Images'],
  ['documents', 'Documents'],
  ['archives', 'Archives'],
  ['received', 'Received'],
];

function matches(item, filter, search) {
  if (filter === 'received' && item.origin !== 'received') return false;
  const kinds = { videos: 'video', music: 'audio', images: 'image', documents: 'document', archives: 'archive' };
  if (kinds[filter] && item.kind !== kinds[filter]) return false;
  if (search && !item.name.toLowerCase().includes(search.toLowerCase())) return false;
  return true;
}

export function render(ctx) {
  const { s, ui } = ctx;
  const filter = ui.libraryFilter || 'all';
  const search = ui.librarySearch || '';
  const items = s.library.filter((item) => matches(item, filter, search));
  const selected = ui.selectedIds || [];
  const selectedItems = s.library.filter((i) => selected.includes(i.id));
  const selectedSize = selectedItems.reduce((sum, i) => sum + i.size, 0);
  const libraryEmpty = s.library.length === 0;

  const rows = items
    .map((item) => {
      const isSelected = selected.includes(item.id);
      return `<button type="button" class="list-row" data-action="library-select" data-id="${item.id}"
          aria-pressed="${isSelected}">
        <span class="row-icon">${icon(fileIcon(item.kind), 24)}</span>
        <span class="row-body">
          <span class="row-title" title="${escapeHtml(item.name)}">${escapeHtml(truncateName(item.name))}</span>
          <span class="row-meta">${escapeHtml(item.source)} · ${fmtSize(item.size)} · ${escapeHtml(item.when)}</span>
        </span>
        <span class="row-trailing">
          <span class="check-box select-wide" aria-hidden="true"
            style="${isSelected ? 'background:var(--accent);border-color:var(--accent);color:var(--on-accent)' : ''}">
            ${icon('check', 16)}
          </span>
        </span>
      </button>`;
    })
    .join('');

  const noResults = `<div class="section">
      <span class="ink-secondary">${icon('search', 32)}</span>
      <h2 class="t-title">No files match “${escapeHtml(search)}”.</h2>
      <p class="t-body ink-secondary prose">Try a shorter name.</p>
      <div><button type="button" class="btn btn-quiet" data-action="clear-search">Clear search</button></div>
    </div>`;

  const empty = `<div class="section">
      <span class="ink-secondary">${icon('folder-open', 32)}</span>
      <h2 class="t-title">Nothing here yet.</h2>
      <p class="t-body ink-secondary prose">Files you download or receive will appear here.</p>
      <div><button type="button" class="btn btn-primary" data-action="library-start">Send or receive a file</button></div>
    </div>`;

  return `
    <div class="page-head">
      <h1 class="t-headline">Find, play, and send your files</h1>
      <p class="support-line">128 files · 14.2 GB</p>
    </div>
    ${
      ctx.ui.share
        ? `<div class="banner" role="status"><span class="banner-body">Web share: <a href="${escapeHtml(ctx.ui.share.url)}">${escapeHtml(ctx.ui.share.url)}</a> · code ${escapeHtml(ctx.ui.share.pin)}</span><button class="btn btn-quiet btn-sm" data-action="webshare-stop">Stop</button></div>`
        : ''
    }
    <label class="field-label" for="library-search">Search your files</label>
    <div class="search-field">
      ${icon('search', 24)}
      <input class="input" type="search" id="library-search" data-model="librarySearch" data-focus-id="library-search"
        aria-label="Search your files" value="${escapeHtml(search)}" autocomplete="off">
    </div>
    <div class="chip-row" role="group" aria-label="Filters">
      ${FILTERS.map(
        ([value, label]) => `<button type="button" class="chip${filter === value ? ' is-selected' : ''}"
          data-action="library-filter" data-filter="${value}" aria-pressed="${filter === value}">
          ${filter === value ? icon('check', 16) : ''}${label}
        </button>`,
      ).join('')}
    </div>
    ${
      selected.length
        ? `<div class="selection-bar">
            <span class="selection-text">${selected.length} selected · ${fmtSize(selectedSize)}</span>
            <button type="button" class="btn btn-primary btn-sm" data-action="library-send">Send</button>
            <button type="button" class="btn btn-quiet btn-sm" data-action="library-share">Share</button>
            <button type="button" class="btn btn-quiet btn-sm" data-action="library-delete">Delete</button>
          </div>`
        : ''
    }
    ${
      libraryEmpty
        ? empty
        : items.length === 0
          ? search
            ? noResults
            : `<div class="section">
                <h2 class="t-title">Nothing in this filter</h2>
                <p class="t-body ink-secondary">Choose All to see every file.</p>
              </div>`
          : `<div class="list">${rows}</div>`
    }
  `;
}

export const actions = {
  'webshare-stop': async (ctx) => {
    await ctx.invoke('webshare.stop', {});
    ctx.app.ui.share = null;
    ctx.toast('Web share stopped');
    ctx.render();
  },

  'library-filter': (ctx, el) => {
    ctx.app.ui.libraryFilter = el.dataset.filter;
    ctx.render();
  },

  'clear-search': (ctx) => {
    ctx.app.ui.librarySearch = '';
    ctx.render();
  },

  'library-select': (ctx, el) => {
    const id = el.dataset.id;
    const selected = new Set(ctx.ui.selectedIds || []);
    if (selected.has(id)) selected.delete(id);
    else selected.add(id);
    ctx.app.ui.selectedIds = [...selected];
    ctx.render();
  },

  'library-send': (ctx) => {
    const selected = ctx.ui.selectedIds || [];
    const items = ctx.s.library.filter((i) => selected.includes(i.id));
    if (!items.length) return;
    ctx.app.ui.selection = items.map((i) => ({ id: i.id, libraryId: i.id, name: i.name, kind: i.kind, size: i.size, path: i.path }));
    ctx.app.ui.selectedIds = [];
    ctx.app.ui.pickTarget = true;
    ctx.go('nearby');
    ctx.toast('Choose a device to send to');
  },

  'library-share': async (ctx) => {
    const ids = ctx.ui.selectedIds || [];
    if (!ids.length) return;
    const items = ctx.s.library.filter((item) => ids.includes(item.id) && (item.path || item.id)).map((item) => ({
      ...(item.path ? { path: item.path } : { id: item.id }), name: item.name,
    }));
    if (!items.length) {
      ctx.toast('No shareable library files selected');
      return;
    }
    const share = await ctx.invoke('webshare.start', { files: items });
    ctx.app.ui.selectedIds = [];
    ctx.app.ui.share = share;
    ctx.toast(`Web share ${share.url} · code ${share.pin}`);
    ctx.render();
  },

  'library-delete': (ctx) => {
    ctx.app.ui.deleteIds = ctx.ui.selectedIds || [];
    if (!ctx.app.ui.deleteIds.length) return;
    ctx.openSheet('delete');
  },

  'library-start': (ctx) => {
    ctx.go('nearby');
    ctx.openSheet('choose');
  },
};
