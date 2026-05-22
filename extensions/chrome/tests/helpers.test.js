import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { createBridgeEnv } from './setup.js';

let env;

beforeEach(() => {
  env = createBridgeEnv();
});

afterEach(() => {
  env.dom.window.close();
});

describe('inferRole (via findElements role matching)', () => {
  it('infers button role from <button>', () => {
    env.window.document.body.innerHTML = '<button>Click</button>';
    const results = env.bridge.findElements({ role: 'button' });
    expect(results.length).toBeGreaterThan(0);
    expect(results[0].role).toBe('button');
  });

  it('infers link role from <a>', () => {
    env.window.document.body.innerHTML = '<a href="#">Link</a>';
    const results = env.bridge.findElements({ role: 'link' });
    expect(results.length).toBe(1);
  });

  it('infers textbox role from <input>', () => {
    env.window.document.body.innerHTML = '<input type="text" />';
    const results = env.bridge.findElements({ role: 'textbox' });
    expect(results.length).toBe(1);
  });

  it('infers checkbox from input[type=checkbox]', () => {
    env.window.document.body.innerHTML = '<input type="checkbox" />';
    const results = env.bridge.findElements({ role: 'checkbox' });
    expect(results.length).toBe(1);
  });

  it('infers radio from input[type=radio]', () => {
    env.window.document.body.innerHTML = '<input type="radio" />';
    const results = env.bridge.findElements({ role: 'radio' });
    expect(results.length).toBe(1);
  });

  it('infers button from input[type=submit]', () => {
    env.window.document.body.innerHTML = '<input type="submit" value="Go" />';
    const results = env.bridge.findElements({ role: 'button' });
    expect(results.length).toBe(1);
  });

  it('infers heading from h1-h6', () => {
    env.window.document.body.innerHTML = '<h1>Title</h1><h3>Sub</h3>';
    const results = env.bridge.findElements({ role: 'heading' });
    expect(results.length).toBe(2);
  });

  it('infers navigation from <nav>', () => {
    env.window.document.body.innerHTML = '<nav><a href="#">Home</a></nav>';
    const results = env.bridge.findElements({ role: 'navigation' });
    expect(results.length).toBe(1);
  });

  it('infers list roles', () => {
    env.window.document.body.innerHTML = '<ul><li>Item</li></ul>';
    const lists = env.bridge.findElements({ role: 'list' });
    const items = env.bridge.findElements({ role: 'listitem' });
    expect(lists.length).toBe(1);
    expect(items.length).toBe(1);
  });

  it('infers combobox from <select>', () => {
    env.window.document.body.innerHTML = '<select><option>A</option></select>';
    const results = env.bridge.findElements({ role: 'combobox' });
    expect(results.length).toBe(1);
  });
});

describe('color and contrast helpers (via a11y audit)', () => {
  it('audit runs on clean page with no violations', () => {
    env.window.document.body.innerHTML = `
      <html lang="en">
        <h1>Title</h1>
        <img src="logo.png" alt="Logo" />
        <button>Click me</button>
        <a href="#">Link text</a>
      </html>
    `;
    const audit = env.bridge.auditAccessibility();
    expect(audit).toBeDefined();
    expect(audit.summary).toBeDefined();
    expect(typeof audit.summary.total).toBe('number');
    expect(typeof audit.summary.critical).toBe('number');
  });

  it('detects image without alt text', () => {
    env.window.document.body.innerHTML = '<img src="photo.jpg" />';
    const audit = env.bridge.auditAccessibility();
    const imgViolation = audit.violations.find(v => v.rule === 'img-alt');
    expect(imgViolation).toBeDefined();
  });

  it('detects empty button', () => {
    env.window.document.body.innerHTML = '<button></button>';
    const audit = env.bridge.auditAccessibility();
    const btnViolation = audit.violations.find(v => v.rule === 'button-name');
    expect(btnViolation).toBeDefined();
  });

  it('detects empty link', () => {
    env.window.document.body.innerHTML = '<a href="#"></a>';
    const audit = env.bridge.auditAccessibility();
    const linkViolation = audit.violations.find(v => v.rule === 'link-name');
    expect(linkViolation).toBeDefined();
  });

  it('detects heading hierarchy skip', () => {
    env.window.document.body.innerHTML = '<h1>Title</h1><h3>Skipped</h3>';
    const audit = env.bridge.auditAccessibility();
    const headingIssue = [...audit.violations, ...audit.warnings].find(v => v.rule === 'heading-order');
    expect(headingIssue).toBeDefined();
  });
});

describe('describeEl (via diagnostics)', () => {
  it('getDiagnostics returns bridge version and info', () => {
    const diag = env.bridge.getDiagnostics();
    expect(diag).toBeDefined();
    expect(diag.info).toBeDefined();
    expect(diag.info.bridge_version).toBeDefined();
    expect(diag.info.url).toBe('http://localhost:3000/');
  });
});
