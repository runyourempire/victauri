import { readFileSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { JSDOM } from 'jsdom';

const __dirname = dirname(fileURLToPath(import.meta.url));
const BRIDGE_PATH = resolve(__dirname, '..', 'content-main.js');
const bridgeSource = readFileSync(BRIDGE_PATH, 'utf8');

export function createBridgeEnv(html = '<html><head><title>Test</title></head><body></body></html>') {
  const dom = new JSDOM(html, {
    url: 'http://localhost:3000',
    pretendToBeVisual: true,
    runScripts: 'dangerously',
    resources: 'usable',
  });

  const { window } = dom;

  // jsdom doesn't have PerformanceObserver — stub it
  if (!window.PerformanceObserver) {
    window.PerformanceObserver = class {
      observe() {}
      disconnect() {}
    };
  }

  // jsdom doesn't have performance.getEntriesByType — stub minimal
  if (!window.performance.getEntriesByType) {
    window.performance.getEntriesByType = () => [];
  }

  // jsdom getBoundingClientRect returns all zeros — stub non-zero values for actionability
  const origGetBCR = window.HTMLElement.prototype.getBoundingClientRect;
  window.HTMLElement.prototype.getBoundingClientRect = function() {
    return { x: 10, y: 10, width: 100, height: 30, top: 10, right: 110, bottom: 40, left: 10 };
  };

  // jsdom doesn't have elementFromPoint — stub to return the element itself
  if (!window.document.elementFromPoint || window.document.elementFromPoint(0,0) === null) {
    window.document.elementFromPoint = function(x, y) {
      return window.document.body.firstElementChild || window.document.body;
    };
  }

  // Stub getComputedStyle to return basic values jsdom doesn't compute
  const origGetComputedStyle = window.getComputedStyle;
  window.getComputedStyle = function(el) {
    const styles = origGetComputedStyle.call(window, el);
    return new Proxy(styles, {
      get(target, prop) {
        if (prop === 'display') return target.display || 'block';
        if (prop === 'visibility') return target.visibility || 'visible';
        if (prop === 'opacity') return target.opacity || '1';
        if (prop === 'pointerEvents') return target.pointerEvents || 'auto';
        if (prop === 'color') return target.color || 'rgb(0, 0, 0)';
        if (prop === 'backgroundColor') return target.backgroundColor || 'rgb(255, 255, 255)';
        if (prop === 'fontSize') return target.fontSize || '16px';
        if (prop === 'getPropertyValue') return function(name) {
          return target.getPropertyValue(name) || '';
        };
        return target[prop];
      }
    });
  };

  // Capture the MAIN-bridge provenance nonce (audit #2) the way the ISOLATED relay
  // does. The listener is added BEFORE injection so it catches the handshake the
  // bridge fires on load.
  let bridgeNonce = null;
  window.addEventListener('__victauri_handshake', (e) => {
    if (bridgeNonce === null && e.detail && e.detail.nonce) bridgeNonce = e.detail.nonce;
  });

  // Inject the bridge script
  const script = window.document.createElement('script');
  script.textContent = bridgeSource;
  window.document.head.appendChild(script);

  // Fallback in case the load order missed the initial announcement.
  if (bridgeNonce === null) {
    window.dispatchEvent(new window.CustomEvent('__victauri_handshake_req'));
  }

  // Simulate the ISOLATED relay: stamp the nonce onto nonce-less command events so
  // existing tests (which dispatch __victauri_command directly) keep working.
  const origDispatch = window.dispatchEvent.bind(window);
  window.dispatchEvent = function (ev) {
    if (ev && ev.type === '__victauri_command' && ev.detail && ev.detail.nonce == null && bridgeNonce) {
      ev = new window.CustomEvent('__victauri_command', {
        detail: Object.assign({}, ev.detail, { nonce: bridgeNonce }),
      });
    }
    return origDispatch(ev);
  };

  return { dom, window, bridge: window.__VICTAURI__, nonce: bridgeNonce };
}
