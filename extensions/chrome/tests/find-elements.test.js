import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { createBridgeEnv } from './setup.js';

let env;

beforeEach(() => {
  env = createBridgeEnv();
});

afterEach(() => {
  env.dom.window.close();
});

describe('findElements', () => {
  describe('by text', () => {
    it('finds element by text substring', () => {
      env.window.document.body.innerHTML = '<button>Save Changes</button>';
      const results = env.bridge.findElements({ text: 'Save' });
      expect(results.length).toBeGreaterThan(0);
      expect(results[0].text).toContain('Save');
    });

    it('text search is case-insensitive', () => {
      env.window.document.body.innerHTML = '<p>Hello World</p>';
      const results = env.bridge.findElements({ text: 'hello world' });
      expect(results.length).toBeGreaterThan(0);
    });

    it('exact text match when exact=true', () => {
      env.window.document.body.innerHTML = '<span>Save</span><span>Save Changes</span>';
      const results = env.bridge.findElements({ text: 'Save', exact: true });
      expect(results.length).toBe(1);
      expect(results[0].text).toBe('Save');
    });

    it('returns empty for no match', () => {
      env.window.document.body.innerHTML = '<p>Hello</p>';
      const results = env.bridge.findElements({ text: 'Goodbye' });
      expect(results.length).toBe(0);
    });
  });

  describe('by CSS selector', () => {
    it('finds by css class', () => {
      env.window.document.body.innerHTML = '<div class="card"><p>Content</p></div>';
      const results = env.bridge.findElements({ css: '.card' });
      expect(results.length).toBe(1);
      expect(results[0].tag).toBe('div');
    });

    it('finds by css id', () => {
      env.window.document.body.innerHTML = '<input id="email" />';
      const results = env.bridge.findElements({ css: '#email' });
      expect(results.length).toBe(1);
    });

    it('finds by compound selector', () => {
      env.window.document.body.innerHTML = '<nav><a href="#">Home</a><a href="#">About</a></nav>';
      const results = env.bridge.findElements({ css: 'nav a' });
      expect(results.length).toBe(2);
    });

    it('handles invalid CSS gracefully', () => {
      env.window.document.body.innerHTML = '<p>Content</p>';
      const results = env.bridge.findElements({ css: ':::invalid' });
      expect(results.length).toBe(0);
    });
  });

  describe('by role', () => {
    it('finds explicit role', () => {
      env.window.document.body.innerHTML = '<div role="alert">Warning!</div>';
      const results = env.bridge.findElements({ role: 'alert' });
      expect(results.length).toBe(1);
    });

    it('finds implicit role', () => {
      env.window.document.body.innerHTML = '<button>OK</button>';
      const results = env.bridge.findElements({ role: 'button' });
      expect(results.length).toBe(1);
    });
  });

  describe('by test_id', () => {
    it('finds by data-testid', () => {
      env.window.document.body.innerHTML = '<div data-testid="login-form"><input /></div>';
      const results = env.bridge.findElements({ test_id: 'login-form' });
      expect(results.length).toBe(1);
    });

    it('returns empty for nonexistent test_id', () => {
      env.window.document.body.innerHTML = '<div>No test id</div>';
      const results = env.bridge.findElements({ test_id: 'nope' });
      expect(results.length).toBe(0);
    });
  });

  describe('by tag', () => {
    it('finds by tag name', () => {
      env.window.document.body.innerHTML = '<button>A</button><button>B</button><span>C</span>';
      const results = env.bridge.findElements({ tag: 'button' });
      expect(results.length).toBe(2);
    });

    it('tag search is case-insensitive', () => {
      env.window.document.body.innerHTML = '<INPUT type="text" />';
      const results = env.bridge.findElements({ tag: 'input' });
      expect(results.length).toBe(1);
    });
  });

  describe('by accessible name', () => {
    it('finds by aria-label', () => {
      env.window.document.body.innerHTML = '<button aria-label="Close dialog">X</button>';
      const results = env.bridge.findElements({ name: 'Close dialog' });
      expect(results.length).toBe(1);
    });

    it('finds by title attribute', () => {
      env.window.document.body.innerHTML = '<span title="Settings gear icon">*</span>';
      const results = env.bridge.findElements({ name: 'Settings' });
      expect(results.length).toBe(1);
    });

    it('finds by placeholder', () => {
      env.window.document.body.innerHTML = '<input placeholder="Search..." />';
      const results = env.bridge.findElements({ name: 'Search' });
      expect(results.length).toBe(1);
    });
  });

  describe('by placeholder', () => {
    it('finds input by placeholder text', () => {
      env.window.document.body.innerHTML = '<input placeholder="Enter email" />';
      const results = env.bridge.findElements({ placeholder: 'email' });
      expect(results.length).toBe(1);
    });
  });

  describe('by alt text', () => {
    it('finds image by alt', () => {
      env.window.document.body.innerHTML = '<img src="logo.png" alt="Company Logo" />';
      const results = env.bridge.findElements({ alt: 'Company' });
      expect(results.length).toBe(1);
    });
  });

  describe('by label', () => {
    it('finds input by label for attribute', () => {
      env.window.document.body.innerHTML = '<label for="email">Email</label><input id="email" />';
      const results = env.bridge.findElements({ label: 'Email' });
      expect(results.length).toBe(1);
      expect(results[0].tag).toBe('input');
    });

    it('finds input nested in label', () => {
      env.window.document.body.innerHTML = '<label>Username <input type="text" /></label>';
      const results = env.bridge.findElements({ label: 'Username' });
      expect(results.length).toBe(1);
      expect(results[0].tag).toBe('input');
    });
  });

  describe('by enabled state', () => {
    it('filters out disabled when enabled=true', () => {
      env.window.document.body.innerHTML = '<button>Active</button><button disabled>Disabled</button>';
      const results = env.bridge.findElements({ tag: 'button', enabled: true });
      expect(results.length).toBe(1);
      expect(results[0].text).toBe('Active');
    });

    it('finds only disabled when enabled=false', () => {
      env.window.document.body.innerHTML = '<button>Active</button><button disabled>Disabled</button>';
      const results = env.bridge.findElements({ tag: 'button', enabled: false });
      expect(results.length).toBe(1);
      expect(results[0].text).toBe('Disabled');
    });
  });

  describe('combined queries', () => {
    it('combines role and text', () => {
      env.window.document.body.innerHTML = '<button>Save</button><button>Cancel</button>';
      const results = env.bridge.findElements({ role: 'button', text: 'Save' });
      expect(results.length).toBe(1);
      expect(results[0].text).toBe('Save');
    });

    it('combines tag and css', () => {
      env.window.document.body.innerHTML = '<button class="primary">OK</button><button class="secondary">Cancel</button>';
      const results = env.bridge.findElements({ tag: 'button', css: '.primary' });
      expect(results.length).toBe(1);
    });
  });

  describe('max_results', () => {
    it('respects max_results limit', () => {
      env.window.document.body.innerHTML = '<span>1</span><span>2</span><span>3</span><span>4</span><span>5</span>';
      const results = env.bridge.findElements({ tag: 'span', max_results: 2 });
      expect(results.length).toBe(2);
    });

    it('defaults to 10 max results', () => {
      let html = '';
      for (let i = 0; i < 15; i++) html += `<span>Item ${i}</span>`;
      env.window.document.body.innerHTML = html;
      const results = env.bridge.findElements({ tag: 'span' });
      expect(results.length).toBe(10);
    });
  });

  describe('result structure', () => {
    it('returns all expected fields', () => {
      env.window.document.body.innerHTML = '<button id="btn" aria-label="Submit form">Submit</button>';
      const results = env.bridge.findElements({ tag: 'button' });
      expect(results.length).toBe(1);
      const r = results[0];
      expect(r.ref_id).toMatch(/^e\d+$/);
      expect(r.tag).toBe('button');
      expect(r.role).toBe('button');
      expect(r.name).toBe('Submit form');
      expect(r.text).toBe('Submit');
      expect(r.bounds).toBeDefined();
      expect(typeof r.visible).toBe('boolean');
      expect(typeof r.enabled).toBe('boolean');
    });
  });
});
