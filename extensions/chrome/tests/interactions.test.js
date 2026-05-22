import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { createBridgeEnv } from './setup.js';

let env;

beforeEach(() => {
  env = createBridgeEnv();
});

afterEach(() => {
  env.dom.window.close();
});

describe('click', () => {
  it('clicks an element by ref', async () => {
    env.window.document.body.innerHTML = '<button id="btn">Click Me</button>';
    const snap = env.bridge.snapshot('json');
    const btn = findNode(snap.tree, n => n.tag === 'button');
    const result = await env.bridge.click(btn.ref_id);
    expect(result.ok).toBe(true);
  });

  it('dispatches click event', async () => {
    env.window.document.body.innerHTML = '<button id="btn">Click Me</button>';
    let clicked = false;
    env.window.document.getElementById('btn').addEventListener('click', () => { clicked = true; });
    const snap = env.bridge.snapshot('json');
    const btn = findNode(snap.tree, n => n.tag === 'button');
    await env.bridge.click(btn.ref_id);
    expect(clicked).toBe(true);
  });

  it('fails on invalid ref', async () => {
    const result = await env.bridge.click('e99999', 200);
    expect(result.ok).toBe(false);
    expect(result.error).toContain('ref not found');
  });

  it('fails on disabled element', async () => {
    env.window.document.body.innerHTML = '<button disabled>Nope</button>';
    const snap = env.bridge.snapshot('json');
    const btn = findNode(snap.tree, n => n.tag === 'button');
    const result = await env.bridge.click(btn.ref_id, 200);
    expect(result.ok).toBe(false);
    expect(result.error).toContain('disabled');
  });
});

describe('doubleClick', () => {
  it('dispatches dblclick event', async () => {
    env.window.document.body.innerHTML = '<div id="target">Double</div>';
    let dblClicked = false;
    env.window.document.getElementById('target').addEventListener('dblclick', () => { dblClicked = true; });
    const snap = env.bridge.snapshot('json');
    const div = findNode(snap.tree, n => n.tag === 'div');
    await env.bridge.doubleClick(div.ref_id);
    expect(dblClicked).toBe(true);
  });
});

describe('hover', () => {
  it('dispatches mouseover event', async () => {
    env.window.document.body.innerHTML = '<div id="target">Hover me</div>';
    const events = [];
    const el = env.window.document.getElementById('target');
    el.addEventListener('mouseenter', () => events.push('mouseenter'));
    el.addEventListener('mouseover', () => events.push('mouseover'));
    const snap = env.bridge.snapshot('json');
    const div = findNode(snap.tree, n => n.tag === 'div');
    await env.bridge.hover(div.ref_id);
    expect(events).toContain('mouseover');
  });
});

describe('fill', () => {
  it('fills an input value', async () => {
    env.window.document.body.innerHTML = '<input id="inp" type="text" />';
    const snap = env.bridge.snapshot('json');
    const inp = findNode(snap.tree, n => n.tag === 'input');
    const result = await env.bridge.fill(inp.ref_id, 'hello world');
    expect(result.ok).toBe(true);
    expect(env.window.document.getElementById('inp').value).toBe('hello world');
  });

  it('fills a textarea', async () => {
    env.window.document.body.innerHTML = '<textarea id="ta"></textarea>';
    const snap = env.bridge.snapshot('json');
    const ta = findNode(snap.tree, n => n.tag === 'textarea');
    const result = await env.bridge.fill(ta.ref_id, 'multi\nline');
    expect(result.ok).toBe(true);
    expect(env.window.document.getElementById('ta').value).toBe('multi\nline');
  });

  it('fails on non-input element', async () => {
    env.window.document.body.innerHTML = '<div id="d">Not input</div>';
    const snap = env.bridge.snapshot('json');
    const div = findNode(snap.tree, n => n.tag === 'div');
    const result = await env.bridge.fill(div.ref_id, 'text');
    expect(result.ok).toBe(false);
  });

  it('dispatches input event', async () => {
    env.window.document.body.innerHTML = '<input id="inp" type="text" />';
    let inputFired = false;
    env.window.document.getElementById('inp').addEventListener('input', () => { inputFired = true; });
    const snap = env.bridge.snapshot('json');
    const inp = findNode(snap.tree, n => n.tag === 'input');
    await env.bridge.fill(inp.ref_id, 'test');
    expect(inputFired).toBe(true);
  });
});

describe('type', () => {
  it('types characters one by one', async () => {
    env.window.document.body.innerHTML = '<input id="inp" type="text" />';
    const snap = env.bridge.snapshot('json');
    const inp = findNode(snap.tree, n => n.tag === 'input');
    const result = await env.bridge.type(inp.ref_id, 'abc');
    expect(result.ok).toBe(true);
  });

  it('dispatches keydown/keyup per character', async () => {
    env.window.document.body.innerHTML = '<input id="inp" type="text" />';
    const events = [];
    const el = env.window.document.getElementById('inp');
    el.addEventListener('keydown', () => events.push('keydown'));
    el.addEventListener('keyup', () => events.push('keyup'));
    const snap = env.bridge.snapshot('json');
    const inp = findNode(snap.tree, n => n.tag === 'input');
    await env.bridge.type(inp.ref_id, 'x');
    expect(events).toContain('keydown');
    expect(events).toContain('keyup');
  });
});

describe('pressKey', () => {
  it('dispatches key events for named key', () => {
    env.window.document.body.innerHTML = '<input id="inp" type="text" />';
    const events = [];
    const el = env.window.document.getElementById('inp');
    el.addEventListener('keydown', (e) => events.push(e.key));
    el.focus();
    const result = env.bridge.pressKey('Enter');
    expect(result.ok).toBe(true);
    expect(events).toContain('Enter');
  });

  it('handles Tab key', () => {
    const result = env.bridge.pressKey('Tab');
    expect(result.ok).toBe(true);
  });

  it('handles Escape key', () => {
    const result = env.bridge.pressKey('Escape');
    expect(result.ok).toBe(true);
  });

  it('handles modifier combos', () => {
    const result = env.bridge.pressKey('Control+a');
    expect(result.ok).toBe(true);
  });
});

describe('scrollTo', () => {
  it('scrolls element into view (returns ok)', async () => {
    env.window.document.body.innerHTML = '<div id="target">Scroll target</div>';
    // stub scrollIntoView since jsdom doesn't implement it
    const el = env.window.document.getElementById('target');
    el.scrollIntoView = () => {};
    const snap = env.bridge.snapshot('json');
    const div = findNode(snap.tree, n => n.tag === 'div');
    const result = await env.bridge.scrollTo(div.ref_id);
    expect(result.ok).toBe(true);
  });
});

describe('focusElement', () => {
  it('focuses an element', async () => {
    env.window.document.body.innerHTML = '<input id="inp" />';
    const snap = env.bridge.snapshot('json');
    const inp = findNode(snap.tree, n => n.tag === 'input');
    const result = await env.bridge.focusElement(inp.ref_id);
    expect(result.ok).toBe(true);
  });
});

describe('selectOption', () => {
  it('selects option in a select element', async () => {
    env.window.document.body.innerHTML = '<select id="sel"><option value="a">A</option><option value="b">B</option></select>';
    const snap = env.bridge.snapshot('json');
    const sel = findNode(snap.tree, n => n.tag === 'select');
    const result = await env.bridge.selectOption(sel.ref_id, ['b']);
    expect(result.ok).toBe(true);
    expect(env.window.document.getElementById('sel').value).toBe('b');
  });

  it('fails on non-select element', async () => {
    env.window.document.body.innerHTML = '<div id="d">Not select</div>';
    const snap = env.bridge.snapshot('json');
    const div = findNode(snap.tree, n => n.tag === 'div');
    const result = await env.bridge.selectOption(div.ref_id, ['x']);
    expect(result.ok).toBe(false);
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
