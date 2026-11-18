// App chrome: navigation and theme application.

import { icon } from '../icons.js';
import { escapeHtml } from './dom.js';

export const NAV = [
  { route: 'nearby', label: 'Nearby', icon: 'send' },
  { route: 'downloads', label: 'Downloads', icon: 'download' },
  { route: 'browser', label: 'Browser', icon: 'compass' },
  { route: 'library', label: 'Library', icon: 'folder-open' },
  { route: 'devices', label: 'Devices', icon: 'shield-check', expandedOnly: true },
  { route: 'settings', label: 'Settings', icon: 'settings' },
];

export function navHtml(route) {
  const items = NAV.map((item) => {
    const active = item.route === route ? ' is-active' : '';
    const aria = item.route === route ? ' aria-current="page"' : '';
    return `<a class="nav-item${active}" href="#/${item.route}" data-route="${item.route}"
      ${item.expandedOnly ? 'data-expanded-only="true"' : ''} ${aria}>
      ${icon(item.icon, 24)}
      <span class="nav-label">${escapeHtml(item.label)}</span>
    </a>`;
  }).join('');
  return `<nav class="nav" aria-label="Main">
    <div class="nav-brand">DropGrab</div>
    ${items}
  </nav>`;
}

export function applyTheme(settings) {
  const root = document.documentElement;
  const mode = settings.theme || 'system';
  const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
  root.dataset.theme = mode === 'dark' || (mode === 'system' && prefersDark) ? 'dark' : 'light';
  root.dataset.text =
    settings.textSize === 'largest' ? 'largest' : settings.textSize === 'large' ? 'large' : 'default';
}

export function watchSystemTheme(settings) {
  const query = window.matchMedia('(prefers-color-scheme: dark)');
  const onChange = () => {
    if ((settings.theme || 'system') === 'system') applyTheme(settings);
  };
  if (query.addEventListener) query.addEventListener('change', onChange);
  return () => query.removeEventListener('change', onChange);
}
