import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { createBridgeEnv } from './setup.js';

let env;

beforeEach(() => {
  env = createBridgeEnv();
});

afterEach(() => {
  env.dom.window.close();
});

describe('snapshot', () => {
  it('returns compact format by default', () => {
    env.window.document.body.innerHTML = '<div id="main"><p>Hello</p></div>';
    const result = env.bridge.snapshot();
    expect(result.format).toBe('compact');
    expect(typeof result.tree).toBe('string');
    expect(result.tree).toContain('body');
    expect(result.tree).toContain('[e');
  });

  it('returns compact format explicitly', () => {
    env.window.document.body.innerHTML = '<button>Click</button>';
    const result = env.bridge.snapshot('compact');
    expect(result.format).toBe('compact');
    expect(result.tree).toContain('button');
    expect(result.tree).toContain('Click');
  });

  it('returns json format', () => {
    env.window.document.body.innerHTML = '<div><span>Text</span></div>';
    const result = env.bridge.snapshot('json');
    expect(result.format).toBe('json');
    expect(typeof result.tree).toBe('object');
    expect(result.tree.tag).toBe('body');
    expect(result.tree.children.length).toBeGreaterThan(0);
  });

  it('json snapshot has correct structure', () => {
    env.window.document.body.innerHTML = '<button id="btn">OK</button>';
    const result = env.bridge.snapshot('json');
    const btn = result.tree.children.find(c => c.tag === 'button');
    expect(btn).toBeDefined();
    expect(btn.ref_id).toMatch(/^e\d+$/);
    expect(btn.role).toBe('button');
    expect(btn.text).toBe('OK');
    expect(btn.attributes).toBeDefined();
    expect(btn.attributes.id).toBe('btn');
  });

  it('generates ref handles for elements', () => {
    env.window.document.body.innerHTML = '<div><p>A</p><p>B</p></div>';
    const result = env.bridge.snapshot('compact');
    const refs = result.tree.match(/\[e\d+\]/g);
    expect(refs).not.toBeNull();
    expect(refs.length).toBeGreaterThanOrEqual(2);
  });

  it('tracks stale refs', () => {
    env.window.document.body.innerHTML = '<div id="target">Content</div>';
    env.bridge.snapshot();
    env.window.document.body.innerHTML = '';
    const stale = env.bridge.getStaleRefs();
    expect(Array.isArray(stale)).toBe(true);
  });

  it('snapshot of empty body returns minimal tree', () => {
    env.window.document.body.innerHTML = '';
    const result = env.bridge.snapshot('compact');
    expect(result.tree).toContain('body');
  });

  it('deeply nested DOM is traversed', () => {
    env.window.document.body.innerHTML = '<div><div><div><div><div><span>Deep</span></div></div></div></div></div>';
    const result = env.bridge.snapshot('compact');
    expect(result.tree).toContain('Deep');
  });

  it('json snapshot captures attributes', () => {
    env.window.document.body.innerHTML = '<input type="email" data-testid="email-input" placeholder="Email" />';
    const result = env.bridge.snapshot('json');
    const input = findNodeByTag(result.tree, 'input');
    expect(input).toBeDefined();
    expect(input.attributes.type).toBe('email');
    expect(input.attributes['data-testid']).toBe('email-input');
  });

  it('json snapshot includes focusable flag', () => {
    env.window.document.body.innerHTML = '<button>Focus me</button><div>Not focusable</div>';
    const result = env.bridge.snapshot('json');
    const btn = findNodeByTag(result.tree, 'button');
    const div = findNodeByTag(result.tree, 'div');
    expect(btn.focusable).toBe(true);
    expect(div.focusable).toBe(false);
  });

  it('redacts password input values', () => {
    env.window.document.body.innerHTML = '<input type="password" value="secret123" />';
    const result = env.bridge.snapshot('json');
    const input = findNodeByTag(result.tree, 'input');
    expect(input.value).not.toBe('secret123');
  });
});

describe('getRef', () => {
  it('resolves ref to element', () => {
    env.window.document.body.innerHTML = '<button id="btn">OK</button>';
    const snap = env.bridge.snapshot('json');
    const btnNode = findNodeByTag(snap.tree, 'button');
    expect(btnNode).toBeDefined();

    const el = env.bridge.getRef(btnNode.ref_id);
    expect(el).not.toBeNull();
    expect(el.tagName.toLowerCase()).toBe('button');
  });

  it('returns null for nonexistent ref', () => {
    const el = env.bridge.getRef('e99999');
    expect(el).toBeNull();
  });

  it('returns null for stale ref (removed element)', () => {
    env.window.document.body.innerHTML = '<div id="ephemeral">Gone soon</div>';
    const snap = env.bridge.snapshot('json');
    const divNode = findNodeByTag(snap.tree, 'div');
    const refId = divNode.ref_id;

    env.window.document.body.innerHTML = '';
    const el = env.bridge.getRef(refId);
    expect(el).toBeNull();
  });
});

function findNodeByTag(node, tag) {
  if (!node) return null;
  if (node.tag === tag) return node;
  if (node.children) {
    for (const child of node.children) {
      const found = findNodeByTag(child, tag);
      if (found) return found;
    }
  }
  return null;
}
