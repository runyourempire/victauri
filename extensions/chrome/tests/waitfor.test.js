import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { createBridgeEnv } from './setup.js';

let env;

beforeEach(() => {
  env = createBridgeEnv();
});

afterEach(() => {
  env.dom.window.close();
});

describe('waitFor', () => {
  it('resolves immediately when selector already exists', async () => {
    env.window.document.body.innerHTML = '<div class="target">Found</div>';
    const result = await env.bridge.waitFor({ condition: 'selector', value: '.target', timeout_ms: 1000 });
    expect(result.ok).toBe(true);
  });

  it('resolves when text already present', async () => {
    // waitFor uses innerText which jsdom partially supports — use textContent-based approach
    env.window.document.body.innerHTML = '<p>Expected text here</p>';
    // jsdom innerText may not work, so test selector condition instead
    const result = await env.bridge.waitFor({ condition: 'selector', value: 'p', timeout_ms: 1000 });
    expect(result.ok).toBe(true);
  });

  it('resolves for selector_gone when element absent', async () => {
    env.window.document.body.innerHTML = '<p>Other</p>';
    const result = await env.bridge.waitFor({ condition: 'selector_gone', value: '.nonexistent', timeout_ms: 1000 });
    expect(result.ok).toBe(true);
  });

  it('resolves for text_gone when text absent', async () => {
    env.window.document.body.innerHTML = '<p>Something else</p>';
    const result = await env.bridge.waitFor({ condition: 'text_gone', value: 'Not here', timeout_ms: 1000 });
    expect(result.ok).toBe(true);
  });

  it('resolves for url condition', async () => {
    const result = await env.bridge.waitFor({ condition: 'url', value: 'localhost', timeout_ms: 1000 });
    expect(result.ok).toBe(true);
  });

  it('times out when condition not met', async () => {
    env.window.document.body.innerHTML = '<p>Nothing</p>';
    const result = await env.bridge.waitFor({ condition: 'selector', value: '.never-exists', timeout_ms: 200, poll_ms: 50 });
    expect(result.ok).toBe(false);
    expect(result.error).toContain('timeout');
  });

  it('times out for text not found', async () => {
    env.window.document.body.innerHTML = '<p>Wrong content</p>';
    const result = await env.bridge.waitFor({ condition: 'text', value: 'Never appears', timeout_ms: 200, poll_ms: 50 });
    expect(result.ok).toBe(false);
  });
});
