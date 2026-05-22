import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { createBridgeEnv } from './setup.js';

let env;

beforeEach(() => {
  env = createBridgeEnv();
});

afterEach(() => {
  env.dom.window.close();
});

describe('getStyles', () => {
  it('returns computed styles for an element', () => {
    env.window.document.body.innerHTML = '<div id="styled">Styled</div>';
    const snap = env.bridge.snapshot('json');
    const div = findNode(snap.tree, n => n.tag === 'div');
    const result = env.bridge.getStyles(div.ref_id);
    expect(result).toBeDefined();
    expect(result.ref_id).toBe(div.ref_id);
    expect(result.tag).toBe('div');
    expect(result.styles).toBeDefined();
  });

  it('returns specific properties when requested', () => {
    env.window.document.body.innerHTML = '<div id="styled">Styled</div>';
    const snap = env.bridge.snapshot('json');
    const div = findNode(snap.tree, n => n.tag === 'div');
    const result = env.bridge.getStyles(div.ref_id, ['display', 'color']);
    expect(result.styles.display).toBeDefined();
    expect(result.styles.color).toBeDefined();
  });

  it('returns error for invalid ref', () => {
    const result = env.bridge.getStyles('e99999');
    expect(result.error).toBeDefined();
  });
});

describe('getBoundingBoxes', () => {
  it('returns bounds for elements', () => {
    env.window.document.body.innerHTML = '<div id="box">Box</div>';
    const snap = env.bridge.snapshot('json');
    const div = findNode(snap.tree, n => n.tag === 'div');
    const bounds = env.bridge.getBoundingBoxes([div.ref_id]);
    expect(Array.isArray(bounds)).toBe(true);
    expect(bounds.length).toBe(1);
    expect(bounds[0].ref_id).toBe(div.ref_id);
    expect(bounds[0].tag).toBe('div');
  });

  it('includes box model (margin, padding, border)', () => {
    env.window.document.body.innerHTML = '<div>Box</div>';
    const snap = env.bridge.snapshot('json');
    const div = findNode(snap.tree, n => n.tag === 'div');
    const bounds = env.bridge.getBoundingBoxes([div.ref_id]);
    expect(bounds[0].margin).toBeDefined();
    expect(bounds[0].padding).toBeDefined();
    expect(bounds[0].border).toBeDefined();
  });

  it('returns error for invalid refs', () => {
    const bounds = env.bridge.getBoundingBoxes(['e99999']);
    expect(bounds.length).toBe(1);
    expect(bounds[0].error).toBeDefined();
  });
});

describe('highlightElement', () => {
  it('highlights an element', () => {
    env.window.document.body.innerHTML = '<button id="btn">Highlight me</button>';
    const snap = env.bridge.snapshot('json');
    const btn = findNode(snap.tree, n => n.tag === 'button');
    const result = env.bridge.highlightElement(btn.ref_id, 'red', 'test');
    expect(result.ok).toBe(true);
    expect(result.ref_id).toBe(btn.ref_id);
  });

  it('adds overlay to DOM', () => {
    env.window.document.body.innerHTML = '<div>Target</div>';
    const snap = env.bridge.snapshot('json');
    const div = findNode(snap.tree, n => n.tag === 'div');
    env.bridge.highlightElement(div.ref_id, 'blue');
    const overlays = env.window.document.querySelectorAll('.__victauri_highlight__');
    expect(overlays.length).toBe(1);
  });

  it('clearHighlights removes overlays', () => {
    env.window.document.body.innerHTML = '<div>Target</div>';
    const snap = env.bridge.snapshot('json');
    const div = findNode(snap.tree, n => n.tag === 'div');
    env.bridge.highlightElement(div.ref_id, 'blue');
    const result = env.bridge.clearHighlights();
    expect(result.ok).toBe(true);
    const overlays = env.window.document.querySelectorAll('.__victauri_highlight__');
    expect(overlays.length).toBe(0);
  });
});

describe('injectCss', () => {
  it('injects CSS into the page', () => {
    const result = env.bridge.injectCss('body { background: red; }');
    expect(result.ok).toBe(true);
    expect(result.length).toBeGreaterThan(0);
    const style = env.window.document.getElementById('__victauri_injected_css__');
    expect(style).not.toBeNull();
  });

  it('removeInjectedCss removes it', () => {
    env.bridge.injectCss('body { color: blue; }');
    const result = env.bridge.removeInjectedCss();
    expect(result.ok).toBe(true);
    expect(result.removed).toBe(true);
    const style = env.window.document.getElementById('__victauri_injected_css__');
    expect(style).toBeNull();
  });

  it('removeInjectedCss when nothing injected', () => {
    const result = env.bridge.removeInjectedCss();
    expect(result.ok).toBe(true);
    expect(result.removed).toBe(false);
  });
});

function findNode(node, predicate) {
  if (!node) return null;
  if (predicate(node)) return node;
  if (node.children) {
    for (const child of node.children) {
      const found = findNode(child, predicate);
      if (found) return found;
    }
  }
  return null;
}
