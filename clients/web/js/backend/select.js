// Real backend when explicitly configured; mock remains the default dev backend.
import { MockBackend } from './mock/backend.js';
import { RealBackend, getUiConfig } from './backend/client.js';

const config = getUiConfig();
const useRealBackend = config.mode === 'real' || Boolean(config.token);
const backend = useRealBackend ? new RealBackend(config) : new MockBackend();
const REAL_BACKEND = useRealBackend;
export { backend, REAL_BACKEND };
