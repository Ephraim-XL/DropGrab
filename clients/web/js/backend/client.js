// Thin browser client for the dropgrabd loopback JSON-RPC and SSE APIs.
const DEFAULT_BASE_URL = 'http://127.0.0.1:53319';
const CONFIG_KEY = 'dropgrab.control.v1';
const API_VERSION = '1.0';

function readConfig() {
  let stored = {};
  try { stored = JSON.parse(localStorage.getItem(CONFIG_KEY) || '{}'); } catch { /* use injected config */ }
  const injected = globalThis.DROPGRAB_CONFIG || {};
  return {
    baseUrl: injected.baseUrl || stored.baseUrl || DEFAULT_BASE_URL,
    token: injected.token || stored.token || '',
    events: injected.events !== false,
    mode: injected.mode || stored.mode || '',
    fetchImpl: injected.fetchImpl,
  };
}

export class RealBackend {
  constructor(options = {}) {
    const config = options.baseUrl ? options : readConfig();
    this.baseUrl = String(config.baseUrl || DEFAULT_BASE_URL).replace(/\/+$/, '');
    this.token = config.token || '';
    this.fetch = config.fetchImpl || globalThis.fetch.bind(globalThis);
    this.eventsEnabled = config.events !== false;
    this.listeners = new Set();
    this.seq = 0;
    this.requestId = 0;
    this.state = {
      apiVersion: API_VERSION,
      backend: { version: 'dropgrabd', mode: 'desktop daemon', port: 0 },
      self: { id: '', name: 'DropGrab', type: 'computer', os: '', fingerprint: '', visibility: 'everyone' },
      devices: [], downloads: [], library: [], recent: [], detected: [],
      settings: {}, session: null, incoming: null, pairing: null, share: null, initialized: false,
    };
    this.closed = false;
    this.initPromise = null;
    this.eventsAbort = null;
    this.subscribed = false;
    this.pendingEvents = [];
    this.currentlyInitializing = false;
  }

  subscribe(fn) {
    this.listeners.add(fn);
    if (!this.subscribed && this.eventsEnabled) {
      this.subscribed = true;
      this.eventsAbort = new AbortController();
      this.readEvents(this.eventsAbort.signal).catch((error) => {
        if (!this.closed && error.name !== 'AbortError') console.warn('DropGrab event stream disconnected', error);
      });
    }
    return () => this.listeners.delete(fn);
  }

  emit(event) {
    if (Number.isFinite(event.seq)) this.seq = Math.max(this.seq, event.seq);
    for (const fn of this.listeners) fn(event);
  }

  async readEvents(signal) {
    while (!signal.aborted && !this.closed) {
      try {
        const response = await this.fetch(`${this.baseUrl}/events?after=${this.seq}`, {
          headers: this.headers(), signal,
        });
        if (!response.ok || !response.body) throw new Error(`events HTTP ${response.status}`);
        const reader = response.body.getReader();
        const decoder = new TextDecoder();
        let buffer = '';
        while (!signal.aborted) {
          const { value, done } = await reader.read();
          if (done) break;
          buffer += decoder.decode(value, { stream: true });
          let boundary;
          while ((boundary = buffer.search(/\r?\n\r?\n/)) >= 0) {
            const separator = buffer.slice(boundary).match(/^\r?\n\r?\n/)[0];
            const frame = buffer.slice(0, boundary);
            buffer = buffer.slice(boundary + separator.length);
            const data = frame.split(/\r?\n/).filter((line) => line.startsWith('data:'))
              .map((line) => line.slice(5).trim()).join('\n');
            if (!data) continue;
            try { this.applyEvent(JSON.parse(data)); } catch (error) { console.warn('Invalid DropGrab event', error); }
          }
        }
      } catch (error) {
        if (signal.aborted) return;
        await new Promise((resolve) => setTimeout(resolve, 1500));
      }
    }
  }

  headers() {
    if (!this.token) throw new Error('DropGrab API token is not configured; set DROPGRAB_CONFIG or dropgrab.control.v1');
    return { Authorization: `Bearer ${this.token}` };
  }

  async rpc(method, params = {}) {
    const id = ++this.requestId;
    const response = await this.fetch(`${this.baseUrl}/rpc`, {
      method: 'POST',
      headers: { ...this.headers(), 'Content-Type': 'application/json' },
      body: JSON.stringify({ id, method, params }),
    });
    if (!response.ok) {
      const message = await response.text();
      const error = new Error(`HTTP_${response.status}: ${message}`);
      error.code = `HTTP_${response.status}`;
      throw error;
    }
    const envelope = await response.json();
    if (envelope.error) {
      const error = new Error(`${envelope.error.code}: ${envelope.error.message}`);
      error.code = envelope.error.code;
      error.details = envelope.error.data;
      throw error;
    }
    return envelope.result;
  }

  async invoke(method, params = {}) {
    const result = await this.rpc(method, params);
    if (method === 'backend.info' && result?.device) {
      this.state.backend = { ...result, port: result.port };
      this.state.apiVersion = result.apiVersion;
      this.state.self = {
        id: result.device.id, name: result.device.name, type: result.device.type,
        os: '', fingerprint: result.device.fingerprint, visibility: result.device.visibility,
      };
    } else if (method === 'devices.list' && result?.devices) this.state.devices = result.devices;
    else if (method === 'settings.get') {
      this.state.settings = result.settings || {};
      this.state.self.name = this.state.settings.deviceName || this.state.self.name;
      this.state.self.visibility = this.state.settings.visibility || this.state.self.visibility;
    } else if (method === 'settings.set') {
      this.state.settings = result.settings || { ...this.state.settings, ...params };
      this.state.self.name = this.state.settings.deviceName || this.state.self.name;
      this.state.self.visibility = this.state.settings.visibility || this.state.self.visibility;
    } else if (method === 'downloads.list') this.state.downloads = result.jobs || [];
    else if (method === 'library.query') this.state.library = result.items || [];
    else if (method === 'session.history') this.state.recent = this.mapRecent(result.sessions);
    else if (method === 'session.list') this.state.session = this.toSession((result.sessions || [])[0]);
    else if (method === 'pair.qr_create') this.state.pairing = result;
    else if (method === 'session.send' || method === 'transfer.start') this.state.session = { ...result, direction: 'out', state: result.state, files: params.items || [], total: 0, done: 0, peerName: this.state.devices.find((d) => d.id === params.peer)?.name || params.peer };
    else if (method === 'session.respond' && params.accept) this.state.incoming = null;
    else if (method === 'webshare.start') this.state.share = result;
    else if (method === 'webshare.stop') this.state.share = null;
    return result;
  }

  async initialize() {
    if (this.initPromise) return this.initPromise;
    this.initPromise = (async () => {
      const [info, deviceResult, settingsResult, downloadsResult, libraryResult, recentResult, sessionResult] = await Promise.all([
        this.rpc('backend.info'), this.rpc('devices.list'), this.rpc('settings.get'),
        this.rpc('downloads.list'), this.rpc('library.query', { filter: 'all', search: '' }),
        this.rpc('session.history'), this.rpc('session.list'),
      ]);
      this.state.apiVersion = info.apiVersion;
      this.state.backend = { ...info, port: info.port };
      this.state.self = {
        id: info.device.id, name: info.device.name, type: info.device.type,
        os: '', fingerprint: info.device.fingerprint, visibility: info.device.visibility,
      };
      this.state.devices = deviceResult.devices || [];
      this.state.settings = settingsResult.settings || {};
      this.state.downloads = downloadsResult.jobs || [];
      this.state.library = libraryResult.items || [];
      this.state.recent = this.mapRecent(recentResult.sessions);
      this.state.session = this.toSession((sessionResult.sessions || [])[0]);
      this.state.initialized = true;
      this.currentlyInitializing = true;
      for (const event of this.pendingEvents.splice(0)) this.applyEvent(event);
      this.currentlyInitializing = false;
    })().catch((error) => { this.currentlyInitializing = false; this.initPromise = null; throw error; });
    return this.initPromise;
  }

  toSession(row) {
    return row ? {
      ...row, peer: row.peerName, total: row.total ?? row.totalBytes ?? 0,
      done: row.done ?? row.doneBytes ?? 0, files: row.files || [], eta_s: null,
    } : null;
  }

  mapRecent(rows) {
    return (rows || []).map((row) => ({
      id: row.sid, name: row.peerName || 'Transfer', kind: 'document',
      direction: row.direction === 'out' ? 'sent' : 'received', peer: row.peerName || '',
      size: row.totalBytes || 0, when: row.startedAt ? new Date(row.startedAt * 1000).toLocaleString() : '',
    }));
  }

  applyEvent(event) {
    if (!this.state.initialized && !this.currentlyInitializing && event.type !== 'resync') {
      this.pendingEvents.push(event);
      this.initialize().catch(() => {});
      return;
    }
    const data = event.data || {};
    switch (event.type) {
      case 'peer.up': case 'peer.down': case 'device.changed':
        this.rpc('devices.list').then((r) => { this.state.devices = r.devices || []; this.notifyState(); }).catch(() => {});
        break;
      case 'session.offered':
        this.state.incoming = data;
        this.notifyState();
        break;
      case 'session.state': case 'session.progress': case 'session.finished':
        if (event.type === 'session.state' && data.state === 'offered' && data.direction === 'out') {
          this.state.session = { sid: data.sid, direction: 'out', peerName: data.peerName, peerId: data.peerId, files: [], total: 0, done: 0, state: 'offered' };
        }
        if (!this.state.session && event.type === 'session.state' && data.state === 'offered') {
          this.state.session = { sid: data.sid, direction: data.direction, peerName: data.peerName, peerId: data.peerId, files: [], total: 0, done: 0, state: data.state };
        }
        if (this.state.session?.sid === data.sid) {
          this.state.session = { ...(this.state.session || {}), ...data };
          this.notifyState();
        }
        if (event.type === 'session.finished' || (event.type === 'session.state' && data.state !== 'offered')) {
          this.rpc('session.list').then((r) => { this.state.session = this.toSession((r.sessions || [])[0]); this.notifyState(); }).catch(() => {});
          if (event.type === 'session.finished') this.rpc('session.history').then((r) => { this.state.recent = this.mapRecent(r.sessions); this.notifyState(); }).catch(() => {});
        }
        break;
      case 'download.state': case 'download.progress': case 'download.finished':
        this.rpc('downloads.list').then((r) => { this.state.downloads = r.jobs || []; this.notifyState(); }).catch(() => {});
        break;
      case 'library.changed': this.refreshLibrary(); break;
      case 'settings.changed':
        this.rpc('settings.get').then((r) => {
          this.state.settings = r.settings || {};
          this.state.self.name = this.state.settings.deviceName || this.state.self.name;
          this.state.self.visibility = this.state.settings.visibility || this.state.self.visibility;
          this.notifyState();
        }).catch(() => {});
        break;
      case 'pair.updated':
        if (this.state.pairing) this.state.pairing = { ...this.state.pairing, ...data };
        this.notifyState();
        break;
      case 'resync':
        this.initPromise = null;
        this.initialize().then(() => this.emit(event)).catch(() => {});
        break;
      default: break;
    }
    this.emit(event);
  }

  async refreshLibrary() {
    const result = await this.rpc('library.query', { filter: 'all', search: '' });
    this.state.library = result.items || [];
    this.notifyState();
  }

  notifyState() { this.emit({ seq: this.seq, type: 'state.changed', data: {} }); }
  close() { this.closed = true; this.eventsAbort?.abort(); }
  simulateIncoming() { return undefined; }
  setDevices() { return undefined; }
}

export function getUiConfig() { return readConfig(); }
