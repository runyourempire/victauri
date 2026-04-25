pub const INIT_SCRIPT: &str = r#"
(function() {
    if (window.__VICTAURI__) return;

    const refMap = new Map();
    let refCounter = 0;

    window.__VICTAURI__ = {
        version: '0.1.0',

        snapshot: function() {
            refMap.clear();
            refCounter = 0;
            return walkDom(document.body);
        },

        getRef: function(refId) {
            return refMap.get(refId) || null;
        },

        click: function(refId) {
            const el = refMap.get(refId);
            if (!el) return { ok: false, error: 'ref not found: ' + refId };
            el.click();
            return { ok: true };
        },

        fill: function(refId, value) {
            const el = refMap.get(refId);
            if (!el) return { ok: false, error: 'ref not found: ' + refId };
            const nativeSetter = Object.getOwnPropertyDescriptor(
                window.HTMLInputElement.prototype, 'value'
            ).set;
            nativeSetter.call(el, value);
            el.dispatchEvent(new Event('input', { bubbles: true }));
            el.dispatchEvent(new Event('change', { bubbles: true }));
            return { ok: true };
        },

        type: function(refId, text) {
            const el = refMap.get(refId);
            if (!el) return { ok: false, error: 'ref not found: ' + refId };
            el.focus();
            for (const char of text) {
                el.dispatchEvent(new KeyboardEvent('keydown', { key: char, bubbles: true }));
                el.dispatchEvent(new KeyboardEvent('keypress', { key: char, bubbles: true }));
                el.value += char;
                el.dispatchEvent(new Event('input', { bubbles: true }));
                el.dispatchEvent(new KeyboardEvent('keyup', { key: char, bubbles: true }));
            }
            el.dispatchEvent(new Event('change', { bubbles: true }));
            return { ok: true };
        },

        pressKey: function(key) {
            document.activeElement.dispatchEvent(
                new KeyboardEvent('keydown', { key: key, bubbles: true })
            );
            document.activeElement.dispatchEvent(
                new KeyboardEvent('keyup', { key: key, bubbles: true })
            );
            return { ok: true };
        },
    };

    function walkDom(node) {
        if (!node || node.nodeType !== 1) return null;

        const style = window.getComputedStyle(node);
        const visible = style.display !== 'none'
            && style.visibility !== 'hidden'
            && style.opacity !== '0';

        if (!visible) return null;

        const ref_id = 'e' + (refCounter++);
        refMap.set(ref_id, node);

        const rect = node.getBoundingClientRect();
        const role = node.getAttribute('role') || inferRole(node);
        const name = node.getAttribute('aria-label')
            || node.getAttribute('title')
            || node.getAttribute('placeholder')
            || (node.tagName === 'BUTTON' ? node.textContent.trim().substring(0, 80) : null)
            || (node.tagName === 'A' ? node.textContent.trim().substring(0, 80) : null);

        const element = {
            ref_id: ref_id,
            tag: node.tagName.toLowerCase(),
            role: role,
            name: name,
            text: getDirectText(node),
            value: node.value || null,
            enabled: !node.disabled,
            visible: true,
            focusable: node.tabIndex >= 0 || ['INPUT','BUTTON','SELECT','TEXTAREA','A'].includes(node.tagName),
            bounds: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
            children: [],
            attributes: {}
        };

        const interestingAttrs = ['data-testid', 'id', 'type', 'href', 'src', 'checked', 'selected'];
        for (const attr of interestingAttrs) {
            if (node.hasAttribute(attr)) {
                element.attributes[attr] = node.getAttribute(attr);
            }
        }

        for (const child of node.children) {
            const childEl = walkDom(child);
            if (childEl) element.children.push(childEl);
        }

        return element;
    }

    function inferRole(node) {
        const tag = node.tagName;
        const roles = {
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
            const type = node.getAttribute('type');
            if (type === 'checkbox') return 'checkbox';
            if (type === 'radio') return 'radio';
            if (type === 'range') return 'slider';
            if (type === 'submit' || type === 'button') return 'button';
        }
        return roles[tag] || null;
    }

    function getDirectText(node) {
        let text = '';
        for (const child of node.childNodes) {
            if (child.nodeType === 3) text += child.textContent;
        }
        text = text.trim();
        return text.length > 0 ? text.substring(0, 200) : null;
    }

    // Hook console for capture
    const originalConsole = { log: console.log, warn: console.warn, error: console.error, info: console.info };
    const consoleLogs = [];

    function hookConsole(level) {
        console[level] = function(...args) {
            consoleLogs.push({ level, message: args.map(String).join(' '), timestamp: Date.now() });
            if (consoleLogs.length > 500) consoleLogs.shift();
            originalConsole[level].apply(console, args);
        };
    }

    hookConsole('log');
    hookConsole('warn');
    hookConsole('error');
    hookConsole('info');

    window.__VICTAURI__.getConsoleLogs = function(since) {
        if (since) return consoleLogs.filter(l => l.timestamp >= since);
        return consoleLogs;
    };

    window.__VICTAURI__.clearConsoleLogs = function() {
        consoleLogs.length = 0;
    };

    // DOM mutation tracking
    const mutationLog = [];
    let mutationBatchCount = 0;
    let mutationBatchTimer = null;

    const observer = new MutationObserver(function(mutations) {
        mutationBatchCount += mutations.length;
        if (!mutationBatchTimer) {
            mutationBatchTimer = setTimeout(function() {
                mutationLog.push({
                    count: mutationBatchCount,
                    timestamp: Date.now()
                });
                if (mutationLog.length > 500) mutationLog.shift();
                mutationBatchCount = 0;
                mutationBatchTimer = null;
            }, 100);
        }
    });

    observer.observe(document.documentElement, {
        childList: true,
        subtree: true,
        attributes: true,
        characterData: true,
    });

    window.__VICTAURI__.getMutationLog = function(since) {
        if (since) return mutationLog.filter(function(m) { return m.timestamp >= since; });
        return mutationLog;
    };

    window.__VICTAURI__.clearMutationLog = function() {
        mutationLog.length = 0;
    };

    // Event stream: combined feed for polling
    window.__VICTAURI__.getEventStream = function(since) {
        const events = [];
        const ts = since || 0;

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

        events.sort(function(a, b) { return a.timestamp - b.timestamp; });
        return events;
    };
})();
"#;
