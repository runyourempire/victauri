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

describe('recording lifecycle', () => {
  it('recording_start returns session info', async () => {
    const resp = await sendCommand('recording_start');
    expect(resp.type).toBe('result');
    expect(resp.data.session_id).toBeDefined();
    expect(resp.data.started).toBe(true);
    await sendCommand('recording_stop');
  });

  it('recording_stop returns session data', async () => {
    await sendCommand('recording_start');
    const resp = await sendCommand('recording_stop');
    expect(resp.type).toBe('result');
    expect(resp.data.session_id).toBeDefined();
    expect(resp.data.events).toBeDefined();
  });

  it('recording_stop without start returns error', async () => {
    const resp = await sendCommand('recording_stop');
    expect(resp.type).toBe('result');
    expect(resp.data.error).toBeDefined();
  });
});

describe('checkpoints', () => {
  it('recording_checkpoint creates a checkpoint', async () => {
    await sendCommand('recording_start');
    const resp = await sendCommand('recording_checkpoint', { label: 'test-label' });
    expect(resp.type).toBe('result');
    expect(resp.data.checkpoint_id).toBeDefined();
    expect(resp.data.created).toBe(true);
    await sendCommand('recording_stop');
  });

  it('recording_list_checkpoints returns array', async () => {
    await sendCommand('recording_start');
    await sendCommand('recording_checkpoint', { label: 'cp1' });
    await sendCommand('recording_checkpoint', { label: 'cp2' });
    const resp = await sendCommand('recording_list_checkpoints');
    expect(resp.type).toBe('result');
    expect(Array.isArray(resp.data)).toBe(true);
    expect(resp.data.length).toBeGreaterThanOrEqual(2);
    await sendCommand('recording_stop');
  });

  it('checkpoint without recording returns error', async () => {
    const resp = await sendCommand('recording_checkpoint', { label: 'orphan' });
    expect(resp.type).toBe('result');
    expect(resp.data.error).toBeDefined();
  });
});

describe('events and export', () => {
  it('recording_get_events returns array', async () => {
    await sendCommand('recording_start');
    const resp = await sendCommand('recording_get_events');
    expect(resp.type).toBe('result');
    expect(Array.isArray(resp.data)).toBe(true);
    await sendCommand('recording_stop');
  });

  it('recording_export returns session object', async () => {
    await sendCommand('recording_start');
    await sendCommand('recording_checkpoint', { label: 'export-test' });
    const resp = await sendCommand('recording_export');
    expect(resp.type).toBe('result');
    expect(resp.data.session_id).toBeDefined();
    expect(resp.data.events).toBeDefined();
    expect(resp.data.checkpoints).toBeDefined();
    await sendCommand('recording_stop');
  });
});
