import test from 'node:test';
import assert from 'node:assert/strict';
import { RealBackend } from './client.js';

test('invoke sends authenticated control RPC and returns result', async () => {
  const requests = [];
  const backend = new RealBackend({
    baseUrl: 'http://127.0.0.1:53319/',
    token: 'a'.repeat(64),
    events: false,
    fetchImpl: async (url, options) => {
      requests.push({ url, options });
      return new Response(JSON.stringify({ id: 1, result: { apiVersion: '1.0' } }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      });
    },
  });
  assert.deepEqual(await backend.invoke('backend.info', {}), { apiVersion: '1.0' });
  assert.equal(requests[0].url, 'http://127.0.0.1:53319/rpc');
  assert.equal(requests[0].options.headers.Authorization, `Bearer ${'a'.repeat(64)}`);
  assert.deepEqual(JSON.parse(requests[0].options.body), { id: 1, method: 'backend.info', params: {} });
});

test('initialize loads the daemon snapshots into the UI state shape', async () => {
  const responses = {
    'backend.info': { apiVersion: '1.0', version: 'dropgrabd 0.1', mode: 'desktop daemon', port: 53318, device: { id: 'self1', name: 'Test Box', type: 'computer', fingerprint: 'AB CD', visibility: 'everyone' } },
    'devices.list': { devices: [{ id: 'peer1', name: 'Peer', trust: 'seen', online: false }] },
    'settings.get': { settings: { deviceName: 'Test Box', visibility: 'everyone', theme: 'dark' } },
    'downloads.list': { jobs: [{ id: 'job1', state: 'queued' }] },
    'library.query': { items: [{ id: 'lib1', name: 'File', kind: 'document' }], total: 1 },
    'session.history': { sessions: [{ sid: 'past1', peerName: 'Peer', direction: 'out', totalBytes: 10 }] },
    'session.list': { sessions: [{ sid: 'active1', peerName: 'Peer', direction: 'out', state: 'transferring', total: 20, done: 3 }] },
  };
  const backend = new RealBackend({
    baseUrl: 'http://127.0.0.1:53319', token: 'secret', events: false,
    fetchImpl: async (_url, options) => {
      const { method } = JSON.parse(options.body);
      return new Response(JSON.stringify({ id: 1, result: responses[method] }), { status: 200 });
    },
  });
  await backend.initialize();
  assert.equal(backend.state.self.name, 'Test Box');
  assert.deepEqual(backend.state.devices, responses['devices.list'].devices);
  assert.deepEqual(backend.state.settings, responses['settings.get'].settings);
  assert.deepEqual(backend.state.downloads, responses['downloads.list'].jobs);
  assert.deepEqual(backend.state.library, responses['library.query'].items);
  assert.equal(backend.state.recent[0].id, 'past1');
  assert.equal(backend.state.session.sid, 'active1');
});

test('invoke preserves structured daemon error codes', async () => {
  const backend = new RealBackend({
    baseUrl: 'http://127.0.0.1:53319', token: 'secret', events: false,
    fetchImpl: async () => new Response(JSON.stringify({ id: 1, error: { code: 'INVALID_PARAMS', message: 'url is required' } }), { status: 200 }),
  });
  await assert.rejects(backend.invoke('remote.request_download', {}), (error) => {
    assert.equal(error.code, 'INVALID_PARAMS');
    assert.equal(error.message, 'INVALID_PARAMS: url is required');
    return true;
  });
});

test('invoke refuses to send when the token is missing', async () => {
  const backend = new RealBackend({ baseUrl: 'http://127.0.0.1:53319', events: false });
  await assert.rejects(backend.invoke('backend.info', {}), /API token is not configured/);
});


test('browser file bytes are encoded as base64 for files.stage', async () => {
  const { fileContentBase64 } = await import('../screens/nearby.js');
  const bytes = new TextEncoder().encode('hello DropGrab');
  const file = { arrayBuffer: async () => bytes.buffer };
  assert.equal(await fileContentBase64(file), 'aGVsbG8gRHJvcEdyYWI=');
});

test('transfer.start stores the created session in backend state', async () => {
  const backend = new RealBackend({
    baseUrl: 'http://127.0.0.1:53319', token: 'secret', events: false,
    fetchImpl: async (_url, options) => {
      const { id } = JSON.parse(options.body);
      return new Response(JSON.stringify({ id, result: { sid: 'new-session', state: 'offered' } }), { status: 200 });
    },
  });
  await backend.invoke('transfer.start', { peer: 'peer1', items: [{ stagedId: 'stg_1' }] });
  assert.equal(backend.state.session.sid, 'new-session');
  assert.equal(backend.state.session.state, 'offered');
});

test('RPC ids advance independently of SSE replay sequence', async () => {
  const ids = [];
  const backend = new RealBackend({ baseUrl: 'http://127.0.0.1:53319', token: 'secret', events: false, fetchImpl: async (_url, options) => {
    const body = JSON.parse(options.body); ids.push(body.id);
    return new Response(JSON.stringify({ id: body.id, result: { ok: true } }), { status: 200 });
  } });
  backend.seq = 41;
  await backend.invoke('settings.set', { theme: 'dark' });
  assert.deepEqual(ids, [1]);
  assert.equal(backend.seq, 41);
});
