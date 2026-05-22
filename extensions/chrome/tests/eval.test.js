import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { createBridgeEnv } from './setup.js';

let env;

beforeEach(() => {
  env = createBridgeEnv();
});

afterEach(() => {
  env.dom.window.close();
});

function sendCommand(method, args = {}) {
  return new Promise((resolve) => {
    const id = 'test-' + Math.random().toString(36).slice(2, 8);
    const handler = (event) => {
      if (event.detail && event.detail.id === id) {
        env.window.removeEventListener('__victauri_response', handler);
        resolve(event.detail);
      }
    };
    env.window.addEventListener('__victauri_response', handler);
    env.window.dispatchEvent(new env.window.CustomEvent('__victauri_command', {
      detail: { id, method, args }
    }));
  });
}

describe('eval via command dispatch', () => {
  it('evaluates simple arithmetic', async () => {
    const resp = await sendCommand('eval', { code: '2 + 2' });
    expect(resp.type).toBe('result');
    expect(JSON.parse(resp.data)).toBe(4);
  });

  it('accesses global variables', async () => {
    env.window.__TEST_VAL__ = 42;
    const resp = await sendCommand('eval', { code: '__TEST_VAL__' });
    expect(resp.type).toBe('result');
    expect(JSON.parse(resp.data)).toBe(42);
  });

  it('returns string values', async () => {
    const resp = await sendCommand('eval', { code: '"hello world"' });
    expect(resp.type).toBe('result');
    expect(JSON.parse(resp.data)).toBe('hello world');
  });

  it('returns null', async () => {
    const resp = await sendCommand('eval', { code: 'null' });
    expect(resp.type).toBe('result');
    expect(JSON.parse(resp.data)).toBeNull();
  });

  it('returns undefined as string', async () => {
    const resp = await sendCommand('eval', { code: 'undefined' });
    expect(resp.type).toBe('result');
    expect(resp.data).toBe('undefined');
  });

  it('returns objects', async () => {
    const resp = await sendCommand('eval', { code: '({a: 1, b: "two"})' });
    expect(resp.type).toBe('result');
    expect(JSON.parse(resp.data)).toEqual({ a: 1, b: 'two' });
  });

  it('returns arrays', async () => {
    const resp = await sendCommand('eval', { code: '[1, 2, 3]' });
    expect(resp.type).toBe('result');
    expect(JSON.parse(resp.data)).toEqual([1, 2, 3]);
  });

  it('handles runtime errors', async () => {
    const resp = await sendCommand('eval', { code: 'nonexistentVar.property' });
    expect(resp.type).toBe('error');
    expect(resp.error).toBeDefined();
  });

  it('can manipulate DOM', async () => {
    env.window.document.body.innerHTML = '<div id="target">old</div>';
    await sendCommand('eval', { code: 'document.getElementById("target").textContent = "new"' });
    expect(env.window.document.getElementById('target').textContent).toBe('new');
  });

  it('supports multi-statement code with const/let', async () => {
    const resp = await sendCommand('eval', { code: 'const x = 5; const y = 10; return x + y' });
    expect(resp.type).toBe('result');
    expect(JSON.parse(resp.data)).toBe(15);
  });
});
