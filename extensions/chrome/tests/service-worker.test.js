import { describe, it, expect, beforeEach } from 'vitest';

let chrome;
let sw;

beforeEach(async () => {
  chrome = createChromeMock();
  globalThis.chrome = chrome;
  globalThis.self = { addEventListener: () => {} };
  globalThis.setTimeout = globalThis.setTimeout;
  globalThis.clearTimeout = globalThis.clearTimeout;
  globalThis.setInterval = globalThis.setInterval;
  globalThis.clearInterval = globalThis.clearInterval;
});

describe('service worker module structure', () => {
  it('service-worker.js file exists and is non-empty', async () => {
    const { readFileSync } = await import('fs');
    const { resolve, dirname } = await import('path');
    const { fileURLToPath } = await import('url');
    const __dirname = dirname(fileURLToPath(import.meta.url));
    const source = readFileSync(resolve(__dirname, '..', 'service-worker.js'), 'utf8');
    expect(source.length).toBeGreaterThan(1000);
  });

  it('contains native messaging connection code', async () => {
    const { readFileSync } = await import('fs');
    const { resolve, dirname } = await import('path');
    const { fileURLToPath } = await import('url');
    const __dirname = dirname(fileURLToPath(import.meta.url));
    const source = readFileSync(resolve(__dirname, '..', 'service-worker.js'), 'utf8');
    expect(source).toContain('connectNative');
  });

  it('contains tab lifecycle handlers', async () => {
    const { readFileSync } = await import('fs');
    const { resolve, dirname } = await import('path');
    const { fileURLToPath } = await import('url');
    const __dirname = dirname(fileURLToPath(import.meta.url));
    const source = readFileSync(resolve(__dirname, '..', 'service-worker.js'), 'utf8');
    expect(source).toContain('onActivated');
    expect(source).toContain('onRemoved');
    expect(source).toContain('onUpdated');
  });

  it('does NOT use the debugger/CDP permission (audit #7)', async () => {
    const { readFileSync } = await import('fs');
    const { resolve, dirname } = await import('path');
    const { fileURLToPath } = await import('url');
    const __dirname = dirname(fileURLToPath(import.meta.url));
    const source = readFileSync(resolve(__dirname, '..', 'service-worker.js'), 'utf8');
    // CDP/debugger was dropped — screenshots use captureVisibleTab instead.
    expect(source).not.toContain('chrome.debugger');
    expect(source).toContain('captureVisibleTab');
    // The manifest must not request the `debugger` permission either.
    const manifest = JSON.parse(
      readFileSync(resolve(__dirname, '..', 'manifest.json'), 'utf8')
    );
    expect(manifest.permissions).not.toContain('debugger');
  });

  it('contains screenshot capture logic', async () => {
    const { readFileSync } = await import('fs');
    const { resolve, dirname } = await import('path');
    const { fileURLToPath } = await import('url');
    const __dirname = dirname(fileURLToPath(import.meta.url));
    const source = readFileSync(resolve(__dirname, '..', 'service-worker.js'), 'utf8');
    expect(source).toContain('captureVisibleTab');
    expect(source).toContain('screenshot');
  });

  it('contains command dispatch routing', async () => {
    const { readFileSync } = await import('fs');
    const { resolve, dirname } = await import('path');
    const { fileURLToPath } = await import('url');
    const __dirname = dirname(fileURLToPath(import.meta.url));
    const source = readFileSync(resolve(__dirname, '..', 'service-worker.js'), 'utf8');
    expect(source).toContain('execute');
    expect(source).toContain('navigate');
  });

  it('handles restricted URLs', async () => {
    const { readFileSync } = await import('fs');
    const { resolve, dirname } = await import('path');
    const { fileURLToPath } = await import('url');
    const __dirname = dirname(fileURLToPath(import.meta.url));
    const source = readFileSync(resolve(__dirname, '..', 'service-worker.js'), 'utf8');
    expect(source).toContain('chrome://');
  });

  it('implements reconnection logic', async () => {
    const { readFileSync } = await import('fs');
    const { resolve, dirname } = await import('path');
    const { fileURLToPath } = await import('url');
    const __dirname = dirname(fileURLToPath(import.meta.url));
    const source = readFileSync(resolve(__dirname, '..', 'service-worker.js'), 'utf8');
    expect(source).toContain('reconnect');
  });
});

function createChromeMock() {
  return {
    runtime: {
      connectNative: () => ({
        onMessage: { addListener: () => {} },
        onDisconnect: { addListener: () => {} },
        postMessage: () => {},
      }),
      onMessage: { addListener: () => {} },
      lastError: null,
    },
    tabs: {
      query: () => Promise.resolve([]),
      sendMessage: () => Promise.resolve({}),
      onActivated: { addListener: () => {} },
      onRemoved: { addListener: () => {} },
      onUpdated: { addListener: () => {} },
      onCreated: { addListener: () => {} },
      get: () => Promise.resolve({ id: 1, url: 'http://example.com', title: 'Test' }),
      update: () => Promise.resolve({}),
    },
    debugger: {
      attach: () => Promise.resolve(),
      detach: () => Promise.resolve(),
      sendCommand: () => Promise.resolve({}),
      onDetach: { addListener: () => {} },
    },
    action: {
      setBadgeText: () => {},
      setBadgeBackgroundColor: () => {},
    },
  };
}
