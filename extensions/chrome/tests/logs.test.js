import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { createBridgeEnv } from './setup.js';

let env;

beforeEach(() => {
  env = createBridgeEnv();
});

afterEach(() => {
  env.dom.window.close();
});

describe('console logs', () => {
  it('captures console.log', () => {
    env.window.console.log('test message');
    const logs = env.bridge.getConsoleLogs();
    expect(logs.length).toBeGreaterThan(0);
    const entry = logs.find(l => l.message && l.message.includes('test message'));
    expect(entry).toBeDefined();
  });

  it('captures console.warn', () => {
    env.window.console.warn('warning msg');
    const logs = env.bridge.getConsoleLogs();
    const entry = logs.find(l => l.message && l.message.includes('warning msg'));
    expect(entry).toBeDefined();
    expect(entry.level).toBe('warn');
  });

  it('captures console.error', () => {
    env.window.console.error('error msg');
    const logs = env.bridge.getConsoleLogs();
    const entry = logs.find(l => l.message && l.message.includes('error msg'));
    expect(entry).toBeDefined();
    expect(entry.level).toBe('error');
  });

  it('has timestamp on entries', () => {
    env.window.console.log('timestamped');
    const logs = env.bridge.getConsoleLogs();
    const entry = logs.find(l => l.message && l.message.includes('timestamped'));
    expect(entry).toBeDefined();
    expect(entry.timestamp).toBeDefined();
    expect(typeof entry.timestamp).toBe('number');
  });

  it('clearConsoleLogs removes entries', () => {
    env.window.console.log('will be cleared');
    env.bridge.clearConsoleLogs();
    const logs = env.bridge.getConsoleLogs();
    const entry = logs.find(l => l.message && l.message.includes('will be cleared'));
    expect(entry).toBeUndefined();
  });

  it('since filter returns only recent logs', () => {
    env.window.console.log('old message');
    const now = Date.now();
    env.window.console.log('new message');
    const logs = env.bridge.getConsoleLogs(now - 1);
    expect(logs.length).toBeGreaterThan(0);
  });
});

describe('network log', () => {
  it('getNetworkLog returns array', () => {
    const logs = env.bridge.getNetworkLog();
    expect(Array.isArray(logs)).toBe(true);
  });
});

describe('navigation log', () => {
  it('getNavigationLog returns array', () => {
    const logs = env.bridge.getNavigationLog();
    expect(Array.isArray(logs)).toBe(true);
  });
});

describe('dialog log', () => {
  it('getDialogLog returns array', () => {
    const logs = env.bridge.getDialogLog();
    expect(Array.isArray(logs)).toBe(true);
  });

  it('setDialogAutoResponse configures auto-response', () => {
    const result = env.bridge.setDialogAutoResponse('alert', 'accept', 'OK');
    expect(result.ok).toBe(true);
  });
});

describe('mutation log', () => {
  it('getMutationLog returns array', () => {
    const logs = env.bridge.getMutationLog();
    expect(Array.isArray(logs)).toBe(true);
  });

  it('clearMutationLog resets', () => {
    env.bridge.clearMutationLog();
    const logs = env.bridge.getMutationLog();
    expect(logs.length).toBe(0);
  });
});

describe('event stream', () => {
  it('getEventStream returns array', () => {
    const events = env.bridge.getEventStream();
    expect(Array.isArray(events)).toBe(true);
  });
});
