// Mock control API. Command and event names mirror the Rust backend schema in
// dropgrab-app-design.md section 3.5, so the UI is written against the same
// surface it would use against a real `dropgrabd` over a socket. Nothing here
// touches the network; timers stand in for the engine.

import {
  SELF,
  DEVICES_SEED,
  DOWNLOADS_SEED,
  LIBRARY_SEED,
  RECENT_SEED,
  QUALITY_OPTIONS,
  SETTINGS_DEFAULT,
  INCOMING_FILES,
  DETECTED_ITEMS,
  FILES,
} from './data.js';

const STORAGE_KEY = 'dropgrab.settings.v1';
const TICK_MS = 200;
const MB = 1e6;
const GB = 1e9;

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function clone(value) {
  return typeof structuredClone === 'function' ? structuredClone(value) : JSON.parse(JSON.stringify(value));
}

function loadSettings() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return clone(SETTINGS_DEFAULT);
    return { ...clone(SETTINGS_DEFAULT), ...JSON.parse(raw) };
  } catch {
    return clone(SETTINGS_DEFAULT);
  }
}

function saveSettings(settings) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    /* storage may be unavailable; settings then live for this session only */
  }
}

export class MockBackend {
  constructor() {
    this.state = {
      apiVersion: '1.0',
      backend: { version: 'dropgrabd 1.0.0', mode: 'desktop daemon (mock)', port: 53318 },
      self: clone(SELF),
      devices: clone(DEVICES_SEED),
      downloads: clone(DOWNLOADS_SEED),
      library: clone(LIBRARY_SEED),
      recent: clone(RECENT_SEED),
      detected: clone(DETECTED_ITEMS),
      settings: loadSettings(),
      session: null,
      incoming: null,
      pairing: null,
      share: null,
      skippedFirstRun: false,
    };
    this.seq = 0;
    this.listeners = new Set();
    this.timers = new Set();
    this.interval = setInterval(() => this.tick(), TICK_MS);
  }

  /* ---------- event bus ---------- */

  subscribe(fn) {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  emit(type, data) {
    this.seq += 1;
    const event = { seq: this.seq, type, data };
    for (const fn of this.listeners) fn(event);
    return event;
  }

  snapshot() {
    return this.state;
  }

  /* ---------- simulation ---------- */

  later(fn, ms) {
    const t = setTimeout(() => {
      this.timers.delete(t);
      fn();
    }, ms);
    this.timers.add(t);
    return t;
  }

  peerOf(id) {
    return this.state.devices.find((d) => d.id === id) || null;
  }

  tick() {
    const s = this.state;
    const dt = TICK_MS / 1000;

    for (const job of s.downloads) {
      if (job.state !== 'downloading') continue;
      job.done = Math.min(job.total, job.done + job.bps * dt);
      this.emit('download.progress', {
        id: job.id,
        done: Math.round(job.done),
        total: job.total,
        bps: job.bps,
        eta_s: Math.max(0, Math.round((job.total - job.done) / job.bps)),
      });
      if (job.done >= job.total) this.finishDownload(job);
    }

    const session = s.session;
    if (session && session.state === 'transferring') {
      session.done = Math.min(session.total, session.done + session.bps * dt);
      session.eta_s = Math.max(0, Math.round((session.total - session.done) / session.bps));
      this.emit('session.progress', {
        sid: session.sid,
        done: Math.round(session.done),
        total: session.total,
        bps: session.bps,
        eta_s: Math.max(0, Math.round((session.total - session.done) / session.bps)),
      });
      if (session.done >= session.total) this.finishSession(session);
    }
  }

  finishDownload(job) {
    job.state = 'finished';
    job.bps = 0;
    job.done = job.total;
    job.finishedAt = Date.now();
    this.emit('download.state', { id: job.id, state: job.state, error: null });
    this.emit('download.finished', { id: job.id, name: job.name, size: job.total });
    this.emit('notification.request', {
      kind: 'download-finished',
      title: 'Download finished',
      body: `${job.name} · ${Math.round(job.total / MB)} MB`,
    });
    this.state.library = [
      {
        id: `itm-${job.id}`,
        name: job.name,
        kind: job.kind,
        size: job.total,
        origin: 'download',
        source: 'Downloaded from example.org',
        when: 'today',
      },
      ...this.state.library,
    ];
    this.emit('library.changed', { reason: 'download-finished' });
  }

  finishSession(session) {
    session.state = 'done';
    session.done = session.total;
    session.bps = 0;
    session.eta_s = 0;
    session.durationS = 21;
    session.message =
      session.direction === 'in'
        ? `Received ${session.files.length} files from ${session.peerName} in 21 s`
        : `Sent ${session.files.length} files to ${session.peerName} in 21 s`;
    this.emit('session.state', { sid: session.sid, state: session.state });
    this.emit('session.finished', { sid: session.sid, state: session.state, direction: session.direction });
    if (session.direction === 'out') {
      this.state.recent = [
        {
          id: `rec-${session.sid}`,
          name: session.files[0].name,
          kind: session.files[0].kind,
          direction: 'sent',
          peer: session.peerName,
          size: session.total,
          when: 'just now',
        },
        ...this.state.recent,
      ];
    }
  }

  /* ---------- commands ---------- */

  async invoke(method, params = {}) {
    await sleep(40 + Math.random() * 60);
    const s = this.state;
    switch (method) {
      /* devices */
      case 'devices.list':
        return { devices: clone(s.devices) };

      case 'devices.trust': {
        const d = this.peerOf(params.deviceId);
        if (!d) throw new Error('UNKNOWN_DEVICE');
        d.trust = 'trusted';
        this.emit('device.changed', { deviceId: d.id, trust: d.trust });
        return { deviceId: d.id, trust: d.trust };
      }

      case 'devices.block': {
        const d = this.peerOf(params.deviceId);
        if (!d) throw new Error('UNKNOWN_DEVICE');
        d.trust = 'blocked';
        d.online = false;
        this.emit('device.changed', { deviceId: d.id, trust: d.trust });
        return { deviceId: d.id, trust: d.trust };
      }

      case 'devices.forget': {
        const d = this.peerOf(params.deviceId);
        if (!d) throw new Error('UNKNOWN_DEVICE');
        d.trust = 'seen';
        d.autoAccept = false;
        this.emit('device.changed', { deviceId: d.id, trust: d.trust });
        return { deviceId: d.id, trust: d.trust };
      }

      case 'devices.set_allowance': {
        const d = this.peerOf(params.deviceId);
        if (!d) throw new Error('UNKNOWN_DEVICE');
        if (typeof params.autoAccept === 'boolean') d.autoAccept = params.autoAccept;
        if (typeof params.allowRemote === 'boolean') d.allowRemote = params.allowRemote;
        this.emit('device.changed', { deviceId: d.id, trust: d.trust });
        return { deviceId: d.id, autoAccept: d.autoAccept, allowRemote: d.allowRemote };
      }

      case 'visibility.set':
        s.self.visibility = params.visibility;
        s.settings.visibility = params.visibility;
        saveSettings(s.settings);
        this.emit('settings.changed', { visibility: params.visibility });
        return { visibility: params.visibility };

      /* transfer */
      case 'session.send': {
        const peer = this.peerOf(params.peer);
        if (!peer) throw new Error('NET_UNREACHABLE');
        const files = params.items.map((item) => ({
          fid: item.id,
          name: item.name,
          kind: item.kind || 'document',
          size: item.size,
          done: 0,
        }));
        const total = files.reduce((sum, f) => sum + f.size, 0);
        const sid = `sid-${Math.random().toString(16).slice(2, 8)}`;
        const session = {
          sid,
          direction: 'out',
          peerId: peer.id,
          peerName: peer.name,
          state: 'offered',
          files,
          total,
          done: 0,
          bps: 38.4 * MB,
          eta_s: Math.round(total / (38.4 * MB)),
          startedAt: Date.now(),
        };
        s.session = session;
        this.emit('session.state', { sid, state: 'offered', peerId: peer.id, peerName: peer.name });
        const delay = peer.trust === 'trusted' ? 1200 : 2600;
        this.later(() => {
          if (!s.session || s.session.sid !== sid || s.session.state !== 'offered') return;
          s.session.state = 'transferring';
          s.session.startedAt = Date.now();
          this.emit('session.state', { sid, state: 'transferring' });
        }, delay);
        return { sid, state: 'offered' };
      }

      case 'session.respond': {
        const incoming = s.incoming;
        if (!incoming || incoming.sid !== params.sid) throw new Error('UNKNOWN_SESSION');
        s.incoming = null;
        if (!params.accept) {
          this.emit('session.state', { sid: params.sid, state: 'declined' });
          return { accepted: false };
        }
        if (params.trust) {
          const d = this.peerOf(incoming.peerId);
          if (d) {
            d.trust = 'trusted';
            this.emit('device.changed', { deviceId: d.id, trust: d.trust });
          }
        }
        const files = incoming.files.map((f) => ({ ...f, done: 0 }));
        const total = files.reduce((sum, f) => sum + f.size, 0);
        s.session = {
          sid: params.sid,
          direction: 'in',
          peerId: incoming.peerId,
          peerName: incoming.peerName,
          state: 'transferring',
          files,
          total,
          done: 0,
          bps: 22.6 * MB,
          eta_s: Math.round(total / (22.6 * MB)),
          startedAt: Date.now(),
        };
        this.emit('session.state', { sid: params.sid, state: 'transferring' });
        return { accepted: true };
      }

      case 'session.pause': {
        const session = s.session;
        if (!session) throw new Error('UNKNOWN_SESSION');
        if (session.state !== 'transferring') return { state: session.state };
        session.state = 'paused';
        this.emit('session.state', { sid: session.sid, state: 'paused' });
        return { state: 'paused' };
      }

      case 'session.resume': {
        const session = s.session;
        if (!session) throw new Error('UNKNOWN_SESSION');
        if (session.state !== 'paused') return { state: session.state };
        session.state = 'transferring';
        this.emit('session.state', { sid: session.sid, state: 'transferring' });
        return { state: 'transferring' };
      }

      case 'session.cancel': {
        const session = s.session;
        if (session) {
          session.state = 'cancelled';
          this.emit('session.state', { sid: session.sid, state: 'cancelled' });
          this.emit('session.finished', { sid: session.sid, state: 'cancelled' });
        }
        s.session = null;
        return { cancelled: true };
      }

      case 'session.history':
        return { sessions: clone(s.recent) };

      case 'text.send': {
        const peer = this.peerOf(params.peer);
        if (!peer) throw new Error('NET_UNREACHABLE');
        this.emit('notification.request', {
          kind: 'text-sent',
          title: `Sent the link to ${peer.name}`,
          body: params.text.slice(0, 64),
        });
        return { sent: true };
      }

      case 'pair.qr_create': {
        const pairing = {
          code: '482 917',
          pin: '482917',
          verifyCode: '7316',
          expiresAt: Date.now() + 108000,
          uri: `dropgrab://pair?v=1&id=${SELF.id}&fp=${SELF.fingerprint.replace(/ /g, '')}&pin=482917`,
        };
        s.pairing = pairing;
        this.emit('pair.updated', { code: pairing.code, expiresAt: pairing.expiresAt });
        return pairing;
      }

      case 'pair.qr_scan': {
        await sleep(500);
        return { deviceId: 'dev-maya-mba', name: "Maya's MacBook Air", verifyCode: '7316' };
      }

      /* downloads */
      case 'downloads.probe': {
        const url = params.url || '';
        const isMedia = /watch|video|music|episode|rust|play/i.test(url);
        if (isMedia) {
          return {
            kind: 'media',
            title: /rust/i.test(url) ? 'Rust ownership explained' : 'Lecture 07 Operating Systems',
            duration: /rust/i.test(url) ? '18:42' : '1:12:08',
            page: 'example.org',
            formats: clone(QUALITY_OPTIONS),
            drm: false,
          };
        }
        return {
          kind: 'direct',
          filename: url.split('/').pop() || 'download.bin',
          total: 4.1 * MB,
          drm: false,
          formats: [],
        };
      }

      case 'downloads.add': {
        const url = params.url || '';
        const quality = QUALITY_OPTIONS.find((q) => q.id === params.options?.quality) || QUALITY_OPTIONS[1];
        const isMedia = params.kind === 'media';
        const name =
          params.name || url.split('/').pop() || (isMedia ? 'download.mp4' : 'download.bin');
        const job = {
          id: `job-${Math.random().toString(16).slice(2, 8)}`,
          name,
          kind: isMedia ? 'video' : 'document',
          url,
          state: 'probing',
          total: isMedia ? quality.size : 4.1 * MB,
          done: 0,
          bps: isMedia ? 8.2 * MB : 3.1 * MB,
          quality: isMedia ? `${quality.detail} · ${quality.label}` : 'Direct file',
          startedAt: Date.now(),
        };
        s.downloads = [job, ...s.downloads];
        this.emit('download.state', { id: job.id, state: 'probing', error: null });
        this.later(() => {
          job.state = 'downloading';
          this.emit('download.state', { id: job.id, state: 'downloading', error: null });
        }, 900);
        return { id: job.id, state: 'probing' };
      }

      case 'downloads.pause': {
        const job = s.downloads.find((j) => j.id === params.id);
        if (!job) throw new Error('UNKNOWN_JOB');
        job.state = 'paused';
        this.emit('download.state', { id: job.id, state: 'paused', error: null });
        return { id: job.id, state: job.state };
      }

      case 'downloads.resume': {
        const job = s.downloads.find((j) => j.id === params.id);
        if (!job) throw new Error('UNKNOWN_JOB');
        job.state = 'downloading';
        this.emit('download.state', { id: job.id, state: 'downloading', error: null });
        return { id: job.id, state: job.state };
      }

      case 'downloads.cancel': {
        s.downloads = s.downloads.filter((j) => j.id !== params.id);
        this.emit('download.state', { id: params.id, state: 'cancelled', error: null });
        return { cancelled: true };
      }

      case 'downloads.list':
        return { jobs: clone(s.downloads) };

      case 'downloads.reorder':
      case 'downloads.set_priority':
        return { ok: true };

      /* extractor */
      case 'extractor.resolve': {
        await sleep(700);
        return {
          title: 'Rust ownership explained',
          uploader: 'example.org',
          duration: '18:42',
          formats: clone(QUALITY_OPTIONS),
          drm: false,
        };
      }

      case 'extractor.update':
        return { updated: true, version: '2026.11.18' };

      case 'extractor.sites':
        return { sites: ['example.org', 'video.example.org', 'music.example.org'] };

      /* library */
      case 'library.query': {
        let items = clone(s.library);
        const filter = params.filter || 'all';
        const search = (params.search || '').trim().toLowerCase();
        if (filter === 'received') items = items.filter((i) => i.origin === 'received');
        else if (filter !== 'all') {
          const kindMap = {
            videos: 'video',
            music: 'audio',
            images: 'image',
            documents: 'document',
            archives: 'archive',
          };
          if (kindMap[filter]) items = items.filter((i) => i.kind === kindMap[filter]);
        }
        if (search) items = items.filter((i) => i.name.toLowerCase().includes(search));
        return { items, total: s.library.length };
      }

      case 'library.get':
        return { item: clone(s.library.find((i) => i.id === params.id) || null) };

      case 'library.delete': {
        const ids = params.ids || [];
        s.library = s.library.filter((i) => !ids.includes(i.id));
        this.emit('library.changed', { reason: 'deleted', ids });
        return { deleted: ids.length };
      }

      case 'library.move':
        return { ok: true };

      case 'library.duplicates':
        return { groups: [] };

      /* remote and web share */
      case 'remote.request_download':
        return { queued: true, deviceId: params.deviceId };

      case 'remote.list_requests':
        return { requests: [] };

      case 'webshare.start': {
        const share = {
          url: 'https://192.168.1.20:53320/s/k3x9',
          pin: '504193',
          files: [FILES.thesis, FILES.figures, FILES.defense],
          total: 661.8 * MB,
          expiresAt: Date.now() + 552000,
        };
        s.share = share;
        this.emit('webshare.started', { url: share.url });
        return share;
      }

      case 'webshare.stop':
        s.share = null;
        this.emit('webshare.stopped', {});
        return { stopped: true };

      /* system */
      case 'settings.get':
        return { settings: clone(s.settings) };

      case 'settings.set': {
        s.settings = { ...s.settings, ...params };
        saveSettings(s.settings);
        if (params.visibility) s.self.visibility = params.visibility;
        if (params.deviceName) s.self.name = params.deviceName;
        this.emit('settings.changed', clone(params));
        return { settings: clone(s.settings) };
      }

      case 'backend.info':
        return { ...s.backend, apiVersion: s.apiVersion, device: clone(s.self) };

      case 'diagnostics.run':
        return { multicast: true, portBound: true, interfaces: ['wlan0 192.168.1.14'] };

      case 'logs.export':
        return { path: '/tmp/dropgrab-logs.txt' };

      case 'backend.shutdown':
        return { stopping: true };

      default:
        throw new Error(`UNKNOWN_METHOD: ${method}`);
    }
  }

  /* ---------- demo hooks (not part of the control API) ---------- */

  simulateIncoming() {
    if (this.state.incoming) return this.state.incoming;
    const incoming = {
      sid: `sid-${Math.random().toString(16).slice(2, 8)}`,
      direction: 'in',
      peerId: 'dev-maya-mba',
      peerName: "Maya's MacBook Air",
      trust: 'trusted',
      verifyCode: '7316',
      files: INCOMING_FILES.map((f) => ({ ...f, fid: f.id })),
      total: INCOMING_FILES.reduce((sum, f) => sum + f.size, 0),
      savingTo: 'Received',
      executable: false,
      offeredAt: Date.now(),
    };
    this.state.incoming = incoming;
    this.emit('session.offered', incoming);
    this.emit('notification.request', {
      kind: 'incoming',
      title: `${incoming.peerName} wants to send you ${incoming.files.length} files`,
      body: `${Math.round(incoming.total / MB)} MB · Accept or decline`,
    });
    return incoming;
  }

  setDevices(list) {
    const before = new Map(this.state.devices.map((device) => [device.id, device]));
    const next = Array.isArray(list) ? list : [];
    const after = new Set(next.map((device) => device.id));
    this.state.devices = next;
    for (const device of next) {
      if (!before.has(device.id)) this.emit('peer.up', { deviceId: device.id, name: device.name });
    }
    for (const device of before.values()) {
      if (!after.has(device.id)) this.emit('peer.down', { deviceId: device.id, name: device.name });
    }
    this.emit('resync', {});
  }
}

export { MB, GB };
