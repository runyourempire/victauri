import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { createBridgeEnv } from './setup.js';

let env;

beforeEach(() => {
  env = createBridgeEnv();
});

afterEach(() => {
  env.dom.window.close();
});

describe('localStorage', () => {
  it('getLocalStorage returns all items', () => {
    env.window.localStorage.setItem('key1', 'value1');
    env.window.localStorage.setItem('key2', 'value2');
    const storage = env.bridge.getLocalStorage();
    expect(storage.key1).toBe('value1');
    expect(storage.key2).toBe('value2');
  });

  it('setLocalStorage sets a value', () => {
    env.bridge.setLocalStorage('testKey', 'testValue');
    expect(env.window.localStorage.getItem('testKey')).toBe('testValue');
  });

  it('deleteLocalStorage removes a key', () => {
    env.window.localStorage.setItem('toDelete', 'gone');
    env.bridge.deleteLocalStorage('toDelete');
    expect(env.window.localStorage.getItem('toDelete')).toBeNull();
  });

  it('getLocalStorage with key returns single value', () => {
    env.window.localStorage.setItem('specific', 'val');
    const result = env.bridge.getLocalStorage('specific');
    expect(result).toBe('val');
  });

  it('getLocalStorage with missing key returns null', () => {
    const result = env.bridge.getLocalStorage('nonexistent');
    expect(result).toBeNull();
  });

  it('setLocalStorage handles JSON values', () => {
    env.bridge.setLocalStorage('obj', { a: 1 });
    const raw = env.window.localStorage.getItem('obj');
    expect(raw).toBe('{"a":1}');
    const parsed = env.bridge.getLocalStorage('obj');
    expect(parsed).toEqual({ a: 1 });
  });
});

describe('sessionStorage', () => {
  it('getSessionStorage returns all items', () => {
    env.window.sessionStorage.setItem('sk1', 'sv1');
    const storage = env.bridge.getSessionStorage();
    expect(storage.sk1).toBe('sv1');
  });

  it('setSessionStorage sets a value', () => {
    env.bridge.setSessionStorage('sKey', 'sVal');
    expect(env.window.sessionStorage.getItem('sKey')).toBe('sVal');
  });

  it('deleteSessionStorage removes a key', () => {
    env.window.sessionStorage.setItem('sDelete', 'gone');
    env.bridge.deleteSessionStorage('sDelete');
    expect(env.window.sessionStorage.getItem('sDelete')).toBeNull();
  });
});

describe('cookies', () => {
  it('getCookies returns array', () => {
    const cookies = env.bridge.getCookies();
    expect(Array.isArray(cookies)).toBe(true);
  });
});
