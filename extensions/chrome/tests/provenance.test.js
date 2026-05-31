import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { createBridgeEnv } from './setup.js';

// Audit #2: a page script can dispatch __victauri_command, but without the secret
// nonce (established with the ISOLATED relay before page scripts run) the MAIN-world
// bridge must ignore it. setup.js only auto-stamps the nonce onto nonce-LESS events,
// so supplying an explicit wrong nonce simulates a hostile page.
describe('bridge command provenance (audit #2)', () => {
  let env;
  beforeEach(() => {
    env = createBridgeEnv();
  });
  afterEach(() => {
    env.dom.window.close();
  });

  function dispatchRaw(detail) {
    return new Promise((resolve) => {
      const handler = (e) => {
        if (e.detail && e.detail.id === detail.id) {
          env.window.removeEventListener('__victauri_response', handler);
          resolve(e.detail);
        }
      };
      env.window.addEventListener('__victauri_response', handler);
      env.window.dispatchEvent(new env.window.CustomEvent('__victauri_command', { detail }));
      setTimeout(() => {
        env.window.removeEventListener('__victauri_response', handler);
        resolve(null); // no response within the window => command was ignored
      }, 150);
    });
  }

  it('ignores a command carrying a forged nonce', async () => {
    const resp = await dispatchRaw({ id: 'forge1', method: 'snapshot', args: {}, nonce: 'forged' });
    expect(resp).toBeNull();
  });

  it('ignores a command with no nonce at all', async () => {
    // Bypass setup.js auto-stamp by giving an empty-string nonce (non-null).
    const resp = await dispatchRaw({ id: 'forge2', method: 'snapshot', args: {}, nonce: '' });
    expect(resp).toBeNull();
  });

  it('honours a command carrying the real nonce', async () => {
    const resp = await dispatchRaw({ id: 'ok1', method: 'snapshot', args: {}, nonce: env.nonce });
    expect(resp).not.toBeNull();
    expect(resp.type).toBe('result');
  });
});
