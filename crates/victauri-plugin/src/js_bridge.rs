pub const INIT_SCRIPT: &str = r#"
(function() {
    if (window.__VICTAURI__) return;

    var refMap = new Map();
    var refCounter = 0;
    var consoleLogs = [];
    var mutationLog = [];
    var ipcLog = [];
    var ipcCounter = 0;
    var networkLog = [];
    var networkCounter = 0;
    var navigationLog = [];
    var dialogLog = [];

    // ── Public API ───────────────────────────────────────────────────────────

    window.__VICTAURI__ = {
        version: '0.2.0',

        // ── DOM ──────────────────────────────────────────────────────────────

        snapshot: function() {
            refMap.clear();
            refCounter = 0;
            return walkDom(document.body);
        },

        getRef: function(refId) {
            return refMap.get(refId) || null;
        },

        // ── Interactions ─────────────────────────────────────────────────────

        click: function(refId) {
            var el = refMap.get(refId);
            if (!el) return { ok: false, error: 'ref not found: ' + refId };
            el.click();
            return { ok: true };
        },

        doubleClick: function(refId) {
            var el = refMap.get(refId);
            if (!el) return { ok: false, error: 'ref not found: ' + refId };
            el.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
            return { ok: true };
        },

        hover: function(refId) {
            var el = refMap.get(refId);
            if (!el) return { ok: false, error: 'ref not found: ' + refId };
            el.dispatchEvent(new MouseEvent('mouseenter', { bubbles: true }));
            el.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
            return { ok: true };
        },

        fill: function(refId, value) {
            var el = refMap.get(refId);
            if (!el) return { ok: false, error: 'ref not found: ' + refId };
            var proto = el instanceof HTMLTextAreaElement
                ? HTMLTextAreaElement.prototype
                : HTMLInputElement.prototype;
            var desc = Object.getOwnPropertyDescriptor(proto, 'value');
            if (desc && desc.set) {
                desc.set.call(el, value);
            } else {
                el.value = value;
            }
            el.dispatchEvent(new Event('input', { bubbles: true }));
            el.dispatchEvent(new Event('change', { bubbles: true }));
            return { ok: true };
        },

        type: function(refId, text) {
            var el = refMap.get(refId);
            if (!el) return { ok: false, error: 'ref not found: ' + refId };
            el.focus();
            for (var i = 0; i < text.length; i++) {
                var ch = text[i];
                el.dispatchEvent(new KeyboardEvent('keydown', { key: ch, bubbles: true }));
                el.dispatchEvent(new KeyboardEvent('keypress', { key: ch, bubbles: true }));
                if (typeof el.value === 'string') el.value += ch;
                el.dispatchEvent(new Event('input', { bubbles: true }));
                el.dispatchEvent(new KeyboardEvent('keyup', { key: ch, bubbles: true }));
            }
            el.dispatchEvent(new Event('change', { bubbles: true }));
            return { ok: true };
        },

        pressKey: function(key) {
            var target = document.activeElement || document.body;
            target.dispatchEvent(new KeyboardEvent('keydown', { key: key, bubbles: true }));
            target.dispatchEvent(new KeyboardEvent('keyup', { key: key, bubbles: true }));
            return { ok: true };
        },

        selectOption: function(refId, values) {
            var el = refMap.get(refId);
            if (!el) return { ok: false, error: 'ref not found: ' + refId };
            if (el.tagName !== 'SELECT') return { ok: false, error: 'element is not a <select>' };
            var valSet = new Set(values);
            for (var i = 0; i < el.options.length; i++) {
                el.options[i].selected = valSet.has(el.options[i].value);
            }
            el.dispatchEvent(new Event('change', { bubbles: true }));
            return { ok: true };
        },

        scrollTo: function(refId, x, y) {
            if (refId) {
                var el = refMap.get(refId);
                if (!el) return { ok: false, error: 'ref not found: ' + refId };
                el.scrollIntoView({ behavior: 'smooth', block: 'center' });
            } else {
                window.scrollTo({ left: x || 0, top: y || 0, behavior: 'smooth' });
            }
            return { ok: true };
        },

        focusElement: function(refId) {
            var el = refMap.get(refId);
            if (!el) return { ok: false, error: 'ref not found: ' + refId };
            el.focus();
            return { ok: true, tag: el.tagName.toLowerCase() };
        },

        // ── IPC Log ──────────────────────────────────────────────────────────

        getIpcLog: function(limit) {
            if (limit) return ipcLog.slice(-limit);
            return ipcLog;
        },

        clearIpcLog: function() {
            ipcLog.length = 0;
        },

        // ── Console ──────────────────────────────────────────────────────────

        getConsoleLogs: function(since) {
            if (since) return consoleLogs.filter(function(l) { return l.timestamp >= since; });
            return consoleLogs;
        },

        clearConsoleLogs: function() {
            consoleLogs.length = 0;
        },

        // ── Mutations ────────────────────────────────────────────────────────

        getMutationLog: function(since) {
            if (since) return mutationLog.filter(function(m) { return m.timestamp >= since; });
            return mutationLog;
        },

        clearMutationLog: function() {
            mutationLog.length = 0;
        },

        // ── Network ──────────────────────────────────────────────────────────

        getNetworkLog: function(filter, limit) {
            var log = networkLog;
            if (filter) {
                log = log.filter(function(e) { return e.url.indexOf(filter) !== -1; });
            }
            if (limit) log = log.slice(-limit);
            return log;
        },

        clearNetworkLog: function() {
            networkLog.length = 0;
        },

        // ── Storage ──────────────────────────────────────────────────────────

        getLocalStorage: function(key) {
            if (key !== undefined && key !== null) {
                var v = localStorage.getItem(key);
                try { return JSON.parse(v); } catch(e) { return v; }
            }
            var obj = {};
            for (var i = 0; i < localStorage.length; i++) {
                var k = localStorage.key(i);
                var val = localStorage.getItem(k);
                try { obj[k] = JSON.parse(val); } catch(e) { obj[k] = val; }
            }
            return obj;
        },

        setLocalStorage: function(key, value) {
            localStorage.setItem(key, typeof value === 'string' ? value : JSON.stringify(value));
            return { ok: true };
        },

        deleteLocalStorage: function(key) {
            localStorage.removeItem(key);
            return { ok: true };
        },

        getSessionStorage: function(key) {
            if (key !== undefined && key !== null) {
                var v = sessionStorage.getItem(key);
                try { return JSON.parse(v); } catch(e) { return v; }
            }
            var obj = {};
            for (var i = 0; i < sessionStorage.length; i++) {
                var k = sessionStorage.key(i);
                var val = sessionStorage.getItem(k);
                try { obj[k] = JSON.parse(val); } catch(e) { obj[k] = val; }
            }
            return obj;
        },

        setSessionStorage: function(key, value) {
            sessionStorage.setItem(key, typeof value === 'string' ? value : JSON.stringify(value));
            return { ok: true };
        },

        deleteSessionStorage: function(key) {
            sessionStorage.removeItem(key);
            return { ok: true };
        },

        getCookies: function() {
            if (!document.cookie) return [];
            return document.cookie.split(';').map(function(c) {
                var parts = c.trim().split('=');
                return { name: parts[0], value: parts.slice(1).join('=') };
            });
        },

        // ── Navigation ───────────────────────────────────────────────────────

        getNavigationLog: function() {
            return navigationLog;
        },

        navigate: function(url) {
            window.location.href = url;
            return { ok: true };
        },

        navigateBack: function() {
            history.back();
            return { ok: true };
        },

        // ── Dialogs ──────────────────────────────────────────────────────────

        getDialogLog: function() {
            return dialogLog;
        },

        clearDialogLog: function() {
            dialogLog.length = 0;
        },

        setDialogAutoResponse: function(type, action, text) {
            dialogAutoResponses[type] = { action: action, text: text };
            return { ok: true };
        },

        // ── Combined Event Stream ────────────────────────────────────────────

        getEventStream: function(since) {
            var events = [];
            var ts = since || 0;

            consoleLogs.forEach(function(l) {
                if (l.timestamp >= ts) {
                    events.push({ type: 'console', level: l.level, message: l.message, timestamp: l.timestamp });
                }
            });

            mutationLog.forEach(function(m) {
                if (m.timestamp >= ts) {
                    events.push({ type: 'dom_mutation', count: m.count, timestamp: m.timestamp });
                }
            });

            ipcLog.forEach(function(c) {
                if (c.timestamp >= ts) {
                    events.push({ type: 'ipc', command: c.command, status: c.status, duration_ms: c.duration_ms, timestamp: c.timestamp });
                }
            });

            networkLog.forEach(function(n) {
                if (n.timestamp >= ts) {
                    events.push({ type: 'network', method: n.method, url: n.url, status: n.status, duration_ms: n.duration_ms, timestamp: n.timestamp });
                }
            });

            navigationLog.forEach(function(n) {
                if (n.timestamp >= ts) {
                    events.push({ type: 'navigation', url: n.url, nav_type: n.type, timestamp: n.timestamp });
                }
            });

            events.sort(function(a, b) { return a.timestamp - b.timestamp; });
            return events;
        },

        // ── Wait ─────────────────────────────────────────────────────────────

        waitFor: function(opts) {
            return new Promise(function(resolve) {
                var timeout = opts.timeout_ms || 10000;
                var poll = opts.poll_ms || 200;
                var start = Date.now();

                function check() {
                    var elapsed = Date.now() - start;
                    if (elapsed >= timeout) {
                        resolve({ ok: false, error: 'timeout after ' + timeout + 'ms', elapsed_ms: elapsed });
                        return;
                    }

                    var met = false;
                    if (opts.condition === 'text' && opts.value) {
                        met = document.body.innerText.indexOf(opts.value) !== -1;
                    } else if (opts.condition === 'text_gone' && opts.value) {
                        met = document.body.innerText.indexOf(opts.value) === -1;
                    } else if (opts.condition === 'selector' && opts.value) {
                        met = !!document.querySelector(opts.value);
                    } else if (opts.condition === 'selector_gone' && opts.value) {
                        met = !document.querySelector(opts.value);
                    } else if (opts.condition === 'url' && opts.value) {
                        met = window.location.href.indexOf(opts.value) !== -1;
                    } else if (opts.condition === 'ipc_idle') {
                        met = ipcLog.every(function(c) { return c.status !== 'pending'; });
                    } else if (opts.condition === 'network_idle') {
                        met = networkLog.every(function(n) { return n.status !== 'pending'; });
                    }

                    if (met) {
                        resolve({ ok: true, elapsed_ms: Date.now() - start });
                    } else {
                        setTimeout(check, poll);
                    }
                }
                check();
            });
        },
    };

    // ── DOM Walking ──────────────────────────────────────────────────────────

    function walkDom(node) {
        if (!node || node.nodeType !== 1) return null;

        var style = window.getComputedStyle(node);
        var visible = style.display !== 'none'
            && style.visibility !== 'hidden'
            && style.opacity !== '0';

        if (!visible) return null;

        var ref_id = 'e' + (refCounter++);
        refMap.set(ref_id, node);

        var rect = node.getBoundingClientRect();
        var role = node.getAttribute('role') || inferRole(node);
        var name = node.getAttribute('aria-label')
            || node.getAttribute('title')
            || node.getAttribute('placeholder')
            || (node.tagName === 'BUTTON' ? node.textContent.trim().substring(0, 80) : null)
            || (node.tagName === 'A' ? node.textContent.trim().substring(0, 80) : null);

        var element = {
            ref_id: ref_id,
            tag: node.tagName.toLowerCase(),
            role: role,
            name: name,
            text: getDirectText(node),
            value: node.value || null,
            enabled: !node.disabled,
            visible: true,
            focusable: node.tabIndex >= 0 || ['INPUT','BUTTON','SELECT','TEXTAREA','A'].indexOf(node.tagName) !== -1,
            bounds: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
            children: [],
            attributes: {}
        };

        var interestingAttrs = ['data-testid', 'id', 'type', 'href', 'src', 'checked', 'selected'];
        for (var a = 0; a < interestingAttrs.length; a++) {
            if (node.hasAttribute(interestingAttrs[a])) {
                element.attributes[interestingAttrs[a]] = node.getAttribute(interestingAttrs[a]);
            }
        }

        for (var c = 0; c < node.children.length; c++) {
            var childEl = walkDom(node.children[c]);
            if (childEl) element.children.push(childEl);
        }

        return element;
    }

    function inferRole(node) {
        var tag = node.tagName;
        var roles = {
            'BUTTON': 'button', 'A': 'link', 'INPUT': 'textbox',
            'SELECT': 'combobox', 'TEXTAREA': 'textbox', 'IMG': 'img',
            'NAV': 'navigation', 'MAIN': 'main', 'HEADER': 'banner',
            'FOOTER': 'contentinfo', 'ASIDE': 'complementary',
            'H1': 'heading', 'H2': 'heading', 'H3': 'heading',
            'H4': 'heading', 'H5': 'heading', 'H6': 'heading',
            'UL': 'list', 'OL': 'list', 'LI': 'listitem',
            'TABLE': 'table', 'FORM': 'form', 'DIALOG': 'dialog',
        };
        if (tag === 'INPUT') {
            var type = node.getAttribute('type');
            if (type === 'checkbox') return 'checkbox';
            if (type === 'radio') return 'radio';
            if (type === 'range') return 'slider';
            if (type === 'submit' || type === 'button') return 'button';
        }
        return roles[tag] || null;
    }

    function getDirectText(node) {
        var text = '';
        for (var i = 0; i < node.childNodes.length; i++) {
            if (node.childNodes[i].nodeType === 3) text += node.childNodes[i].textContent;
        }
        text = text.trim();
        return text.length > 0 ? text.substring(0, 200) : null;
    }

    // ── Console Hooking ──────────────────────────────────────────────────────

    var originalConsole = {
        log: console.log, warn: console.warn,
        error: console.error, info: console.info, debug: console.debug
    };

    function hookConsole(level) {
        console[level] = function() {
            var args = Array.prototype.slice.call(arguments);
            consoleLogs.push({ level: level, message: args.map(String).join(' '), timestamp: Date.now() });
            if (consoleLogs.length > 1000) consoleLogs.shift();
            originalConsole[level].apply(console, args);
        };
    }

    hookConsole('log');
    hookConsole('warn');
    hookConsole('error');
    hookConsole('info');
    hookConsole('debug');

    // ── Mutation Observer (deferred) ─────────────────────────────────────────

    var mutationBatchCount = 0;
    var mutationBatchTimer = null;

    function startMutationObserver() {
        if (!document.documentElement) return false;
        var observer = new MutationObserver(function(mutations) {
            mutationBatchCount += mutations.length;
            if (!mutationBatchTimer) {
                mutationBatchTimer = setTimeout(function() {
                    mutationLog.push({ count: mutationBatchCount, timestamp: Date.now() });
                    if (mutationLog.length > 500) mutationLog.shift();
                    mutationBatchCount = 0;
                    mutationBatchTimer = null;
                }, 100);
            }
        });
        observer.observe(document.documentElement, {
            childList: true, subtree: true, attributes: true, characterData: true,
        });
        return true;
    }

    if (!startMutationObserver()) {
        document.addEventListener('DOMContentLoaded', startMutationObserver);
    }

    // ── IPC Interception ─────────────────────────────────────────────────────

    function interceptIpc() {
        var tauri = window.__TAURI_INTERNALS__;
        if (!tauri || !tauri.invoke) {
            if (Date.now() - loadTime < 5000) {
                setTimeout(interceptIpc, 50);
            }
            return;
        }
        if (tauri.__victauriPatched) return;
        tauri.__victauriPatched = true;

        var origInvoke = tauri.invoke;
        tauri.invoke = function(cmd, args, options) {
            if (typeof cmd === 'string' && cmd.indexOf('plugin:victauri|') === 0) {
                return origInvoke.call(this, cmd, args, options);
            }
            var id = ++ipcCounter;
            var entry = {
                id: id,
                command: typeof cmd === 'string' ? cmd : String(cmd),
                args: args || {},
                timestamp: Date.now(),
                status: 'pending',
                duration_ms: null,
                result: null,
                error: null,
            };
            ipcLog.push(entry);
            if (ipcLog.length > 2000) ipcLog.shift();

            return origInvoke.call(this, cmd, args, options).then(function(result) {
                entry.status = 'ok';
                try { entry.result = JSON.parse(JSON.stringify(result)); } catch(e) { entry.result = String(result); }
                entry.duration_ms = Date.now() - entry.timestamp;
                return result;
            }, function(err) {
                entry.status = 'error';
                entry.error = String(err);
                entry.duration_ms = Date.now() - entry.timestamp;
                throw err;
            });
        };
    }

    var loadTime = Date.now();
    interceptIpc();

    // ── Network Interception ─────────────────────────────────────────────────

    (function interceptNetwork() {
        // fetch
        var origFetch = window.fetch;
        if (origFetch) {
            window.fetch = function(input, init) {
                var id = ++networkCounter;
                var url = typeof input === 'string' ? input : (input && input.url ? input.url : String(input));
                var method = (init && init.method) || (input && input.method) || 'GET';
                var entry = { id: id, method: method.toUpperCase(), url: url, timestamp: Date.now(), status: 'pending', duration_ms: null };
                networkLog.push(entry);
                if (networkLog.length > 1000) networkLog.shift();

                return origFetch.call(this, input, init).then(function(response) {
                    entry.status = response.status;
                    entry.status_text = response.statusText;
                    entry.duration_ms = Date.now() - entry.timestamp;
                    return response;
                }, function(err) {
                    entry.status = 'error';
                    entry.error = String(err);
                    entry.duration_ms = Date.now() - entry.timestamp;
                    throw err;
                });
            };
        }

        // XMLHttpRequest
        var origOpen = XMLHttpRequest.prototype.open;
        var origSend = XMLHttpRequest.prototype.send;
        XMLHttpRequest.prototype.open = function(method, url) {
            this.__victauri_net = { method: method, url: url };
            return origOpen.apply(this, arguments);
        };
        XMLHttpRequest.prototype.send = function() {
            if (this.__victauri_net) {
                var id = ++networkCounter;
                var entry = {
                    id: id,
                    method: this.__victauri_net.method.toUpperCase(),
                    url: this.__victauri_net.url,
                    timestamp: Date.now(),
                    status: 'pending',
                    duration_ms: null,
                };
                networkLog.push(entry);
                if (networkLog.length > 1000) networkLog.shift();
                var self = this;
                this.addEventListener('load', function() {
                    entry.status = self.status;
                    entry.status_text = self.statusText;
                    entry.duration_ms = Date.now() - entry.timestamp;
                });
                this.addEventListener('error', function() {
                    entry.status = 'error';
                    entry.duration_ms = Date.now() - entry.timestamp;
                });
            }
            return origSend.apply(this, arguments);
        };
    })();

    // ── Navigation Tracking ──────────────────────────────────────────────────

    (function trackNavigation() {
        navigationLog.push({ url: window.location.href, timestamp: Date.now(), type: 'initial' });

        var origPushState = history.pushState;
        var origReplaceState = history.replaceState;
        history.pushState = function() {
            var result = origPushState.apply(this, arguments);
            navigationLog.push({ url: window.location.href, timestamp: Date.now(), type: 'pushState' });
            if (navigationLog.length > 200) navigationLog.shift();
            return result;
        };
        history.replaceState = function() {
            var result = origReplaceState.apply(this, arguments);
            navigationLog.push({ url: window.location.href, timestamp: Date.now(), type: 'replaceState' });
            if (navigationLog.length > 200) navigationLog.shift();
            return result;
        };
        window.addEventListener('popstate', function() {
            navigationLog.push({ url: window.location.href, timestamp: Date.now(), type: 'popstate' });
        });
        window.addEventListener('hashchange', function(e) {
            navigationLog.push({ url: window.location.href, timestamp: Date.now(), type: 'hashchange', old_url: e.oldURL });
        });
    })();

    // ── Dialog Capture ───────────────────────────────────────────────────────

    var dialogAutoResponses = { alert: { action: 'accept' }, confirm: { action: 'accept' }, prompt: { action: 'accept', text: '' } };

    (function captureDialogs() {
        window.alert = function(msg) {
            dialogLog.push({ type: 'alert', message: String(msg || ''), timestamp: Date.now() });
            if (dialogLog.length > 100) dialogLog.shift();
        };
        window.confirm = function(msg) {
            var resp = dialogAutoResponses.confirm;
            var result = resp.action === 'accept';
            dialogLog.push({ type: 'confirm', message: String(msg || ''), timestamp: Date.now(), result: result });
            if (dialogLog.length > 100) dialogLog.shift();
            return result;
        };
        window.prompt = function(msg, defaultValue) {
            var resp = dialogAutoResponses.prompt;
            var result = resp.action === 'accept' ? (resp.text || defaultValue || '') : null;
            dialogLog.push({ type: 'prompt', message: String(msg || ''), timestamp: Date.now(), result: result });
            if (dialogLog.length > 100) dialogLog.shift();
            return result;
        };
    })();
})();
"#;
