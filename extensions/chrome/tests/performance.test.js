import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { createBridgeEnv } from './setup.js';

let env;

beforeEach(() => {
  env = createBridgeEnv();
});

afterEach(() => {
  env.dom.window.close();
});

describe('getPerformanceMetrics', () => {
  it('returns performance data object', () => {
    env.window.document.body.innerHTML = '<div><p>Content</p></div>';
    const metrics = env.bridge.getPerformanceMetrics();
    expect(metrics).toBeDefined();
    expect(typeof metrics).toBe('object');
  });

  it('includes DOM stats', () => {
    env.window.document.body.innerHTML = '<div><p>A</p><p>B</p><span>C</span></div>';
    const metrics = env.bridge.getPerformanceMetrics();
    expect(metrics.dom).toBeDefined();
    expect(metrics.dom.elements).toBeGreaterThan(0);
  });

  it('includes resources section', () => {
    const metrics = env.bridge.getPerformanceMetrics();
    expect(metrics.resources).toBeDefined();
    expect(metrics.resources.total_count).toBeDefined();
  });

  it('DOM element count matches actual DOM', () => {
    env.window.document.body.innerHTML = '<ul><li>1</li><li>2</li><li>3</li></ul>';
    const metrics = env.bridge.getPerformanceMetrics();
    const actualCount = env.window.document.querySelectorAll('*').length;
    expect(metrics.dom.elements).toBe(actualCount);
  });

  it('includes max depth', () => {
    env.window.document.body.innerHTML = '<div><div><div><span>Deep</span></div></div></div>';
    const metrics = env.bridge.getPerformanceMetrics();
    expect(metrics.dom.max_depth).toBeGreaterThanOrEqual(3);
  });

  it('includes paint object', () => {
    const metrics = env.bridge.getPerformanceMetrics();
    expect(metrics.paint).toBeDefined();
  });
});

describe('getDiagnostics', () => {
  it('returns bridge info', () => {
    const diag = env.bridge.getDiagnostics();
    expect(diag).toBeDefined();
    expect(diag.info).toBeDefined();
    expect(diag.info.bridge_version).toBeDefined();
  });

  it('includes URL', () => {
    const diag = env.bridge.getDiagnostics();
    expect(diag.info.url).toBe('http://localhost:3000/');
  });

  it('includes mode', () => {
    const diag = env.bridge.getDiagnostics();
    expect(diag.info.mode).toBe('browser');
  });
});
