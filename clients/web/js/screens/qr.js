// Connect with a QR code.

import { icon } from '../icons.js';
import { escapeHtml, fmtClock } from '../ui/dom.js';

// A deterministic module grid drawn to look like a pairing code. It is a mock,
// not an encoder: it does not carry a scannable payload.
function qrSvg(seed) {
  const N = 25;
  const grid = Array.from({ length: N }, () => Array(N).fill(0));
  let h = 2166136261;
  for (let i = 0; i < seed.length; i += 1) {
    h ^= seed.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  let state = h >>> 0 || 12345;
  const rand = () => {
    state ^= state << 13;
    state >>>= 0;
    state ^= state >>> 17;
    state ^= state << 5;
    state >>>= 0;
    return state / 4294967296;
  };
  const reserved = (x, y) => (x < 8 && y < 8) || (x >= N - 8 && y < 8) || (x < 8 && y >= N - 8);
  for (let y = 0; y < N; y += 1) {
    for (let x = 0; x < N; x += 1) {
      if (reserved(x, y)) continue;
      if (y === 6 || x === 6) {
        grid[y][x] = (x + y) % 2 === 0 ? 1 : 0;
        continue;
      }
      grid[y][x] = rand() > 0.52 ? 1 : 0;
    }
  }
  const finder = (ox, oy) => {
    for (let y = 0; y < 7; y += 1) {
      for (let x = 0; x < 7; x += 1) {
        const edge = x === 0 || y === 0 || x === 6 || y === 6;
        const core = x >= 2 && x <= 4 && y >= 2 && y <= 4;
        grid[oy + y][ox + x] = edge || core ? 1 : 0;
      }
    }
  };
  finder(0, 0);
  finder(N - 7, 0);
  finder(0, N - 7);

  const cells = [];
  for (let y = 0; y < N; y += 1) {
    for (let x = 0; x < N; x += 1) {
      if (grid[y][x]) cells.push(`<rect x="${x}" y="${y}" width="1" height="1"/>`);
    }
  }
  return `<svg viewBox="0 0 ${N} ${N}" shape-rendering="crispEdges" role="img"
    aria-label="Pairing QR code for ${escapeHtml(seed)}" style="background:var(--qr-paper)">
    <g fill="var(--qr-ink)">${cells.join('')}</g>
  </svg>`;
}

export function render(ctx) {
  const { s, ui } = ctx;
  const pairing = s.pairing || { code: '...', expiresAt: Date.now() + 108000, uri: '' };
  const msLeft = Math.max(0, pairing.expiresAt - Date.now());

  const step = ui.qrStep || 'idle';

  const idle = `
    <div class="qr-panel">
      <div class="qr-code">${pairing.uri ? `<a href="${escapeHtml(pairing.uri)}">${qrSvg(pairing.uri)}</a>` : '<p class="t-body-sm">Creating pairing code…</p>'}</div>
      <div class="stack stack-8">
        <p class="t-numeric">Your code: <span class="mono-code">${escapeHtml(pairing.code)}</span></p>
        <p class="t-numeric ink-muted">Expires in ${fmtClock(msLeft)}</p>
      </div>
      <p class="fingerprint">${escapeHtml(s.self.name)} · fingerprint ${escapeHtml(s.self.fingerprint)}</p>
    </div>
    <button type="button" class="btn btn-primary btn-block" data-action="qr-scan">
      Enter another device's QR URI ${icon('scan-line', 24)}
    </button>
    <p class="t-body-sm ink-secondary prose">Both devices need the same code before any file is sent.</p>
    <a class="btn btn-quiet" href="share.html" style="padding:0;height:auto;align-self:flex-start">Open the web share page</a>
  `;

  const scanner = step === 'scanning'
    ? `<div class="section">
        <p class="t-body-sm ink-secondary prose">${escapeHtml(ui.qrMessage || 'Point your camera at the other device’s pairing QR code. You can also paste its URI below.')}</p>
        ${ui.qrCamera ? '<video id="qr-camera" autoplay playsinline muted style="width:100%;max-width:420px;border-radius:var(--radius-md)"></video>' : ''}
        <label class="field-label" for="pairing-uri">Pairing URI</label>
        <input class="input" id="pairing-uri" autocomplete="off" placeholder="dropgrab://pair…">
        <div class="row-16" style="flex-wrap:wrap">
          <button type="button" class="btn btn-primary" data-action="qr-paste">Use pasted URI</button>
          <button type="button" class="btn btn-secondary" data-action="qr-cancel">Cancel</button>
        </div>
      </div>`
    : '';

  const confirm = `
    <div class="section">
      <h2 class="t-title">Does ${escapeHtml(ui.qrPeer?.name || "Maya's MacBook Air")} show ${escapeHtml(ui.qrPeer?.verifyCode || '7316')}?</h2>
      <p class="t-body ink-secondary prose">Check the four digits on the other screen. If they differ, someone else may be answering.</p>
    </div>
    <div class="row-16" style="flex-wrap:wrap">
      <button type="button" class="btn btn-primary" data-action="qr-match">Yes, they match</button>
      <button type="button" class="btn btn-secondary" data-action="qr-cancel">No, cancel</button>
    </div>
  `;

  const connected = `
    <div class="section">
      <p class="row support-line" style="color:var(--primary)">
        ${icon('check', 24)}
        <span class="t-body">Connected to ${escapeHtml(ui.qrPeer?.name || "Maya's MacBook Air")}</span>
      </p>
      <button type="button" class="check-row" role="checkbox" aria-checked="${ui.qrTrust ? 'true' : 'false'}"
        data-action="qr-trust">
        <span class="check-box" aria-hidden="true">${icon('check', 16)}</span>
        <span class="check-label">Trust this device for future transfers</span>
      </button>
    </div>
    <div class="row-16" style="flex-wrap:wrap">
      <button type="button" class="btn btn-primary" data-action="qr-send">Send files</button>
      <button type="button" class="btn btn-quiet" data-action="qr-done">Done</button>
    </div>
  `;

  return `
    <div class="topbar">
      <button type="button" class="icon-btn" data-action="go-nearby" aria-label="Back to Nearby">${icon('chevron-left', 24)}</button>
      <span class="topbar-title">Pair this device</span>
    </div>
    <div class="page-head">
      <h1 class="t-headline">Connect with a QR code</h1>
      <p class="support-line">Use this when devices don't appear on their own</p>
    </div>
    ${step === 'idle' ? idle : step === 'scanning' ? scanner : step === 'confirm' ? confirm : connected}
  `;
}

async function usePairingUri(ctx, uri) {
  const stream = ctx.app.ui.qrStream;
  if (stream) stream.getTracks().forEach((track) => track.stop());
  ctx.app.ui.qrStream = null;
  const peer = await ctx.invoke('pair.qr_scan', { uri });
  ctx.app.ui.qrPeer = peer;
  ctx.app.ui.qrStep = 'confirm';
  ctx.app.ui.qrMessage = '';
  ctx.render();
}

export const actions = {
  'qr-scan': async (ctx) => {
    const BarcodeDetectorApi = globalThis.BarcodeDetector;
    const cameraAvailable = Boolean(BarcodeDetectorApi && navigator.mediaDevices?.getUserMedia);
    ctx.app.ui.qrStep = 'scanning';
    ctx.app.ui.qrMessage = cameraAvailable
      ? 'Camera scan is available. Allow camera access and point it at the other device’s pairing QR code, or paste the pairing URI.'
      : 'Camera scanning is unavailable in this browser. Paste the pairing URI from the other device below.';
    ctx.app.ui.qrCamera = cameraAvailable;
    ctx.render();
    if (!cameraAvailable) return;

    try {
      const detector = new BarcodeDetectorApi({ formats: ['qr_code'] });
      const stream = await navigator.mediaDevices.getUserMedia({ video: { facingMode: 'environment' } });
      ctx.app.ui.qrStream = stream;
      const video = document.getElementById('qr-camera');
      if (!video || ctx.app.ui.qrStep !== 'scanning') {
        stream.getTracks().forEach((track) => track.stop());
        return;
      }
      video.srcObject = stream;
      await video.play();
      const scan = async () => {
        if (ctx.app.ui.qrStep !== 'scanning' || ctx.app.ui.qrStream !== stream) return;
        try {
          const codes = await detector.detect(video);
          if (codes.length && codes[0].rawValue) {
            stream.getTracks().forEach((track) => track.stop());
            ctx.app.ui.qrStream = null;
            await usePairingUri(ctx, codes[0].rawValue);
            return;
          }
        } catch (error) {
          console.warn('QR camera scan failed:', error);
        }
        requestAnimationFrame(scan);
      };
      requestAnimationFrame(scan);
    } catch (error) {
      ctx.app.ui.qrStream?.getTracks().forEach((track) => track.stop());
      ctx.app.ui.qrStream = null;
      ctx.app.ui.qrCamera = false;
      ctx.app.ui.qrMessage = 'Camera access was unavailable. Paste the pairing URI from the other device below.';
      ctx.render();
    }
  },

  'qr-paste': async (ctx) => {
    const uri = document.getElementById('pairing-uri')?.value?.trim();
    if (uri) await usePairingUri(ctx, uri);
  },

  'qr-match': (ctx) => {
    ctx.app.ui.qrStep = 'connected';
    ctx.render();
  },

  'qr-cancel': (ctx) => {
    ctx.app.ui.qrStream?.getTracks().forEach((track) => track.stop());
    ctx.app.ui.qrStream = null;
    ctx.app.ui.qrStep = 'idle';
    ctx.app.ui.qrCamera = false;
    ctx.app.ui.qrTrust = false;
    ctx.render();
  },

  'qr-trust': async (ctx) => {
    ctx.app.ui.qrTrust = !ctx.app.ui.qrTrust;
    if (ctx.app.ui.qrTrust && ctx.app.ui.qrPeer) {
      await ctx.invoke('devices.trust', { deviceId: ctx.app.ui.qrPeer.deviceId });
    }
    ctx.render();
  },

  'qr-done': (ctx) => ctx.go('nearby'),

  'qr-send': (ctx) => {
    ctx.app.ui.qrStep = 'idle';
    ctx.go('nearby');
    ctx.openSheet('choose');
  },
};
