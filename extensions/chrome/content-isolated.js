// Content script — ISOLATED world
// Relays commands between the service worker and the MAIN world content script.
// Has access to chrome.runtime but NOT to page JS globals.

// Secret nonce shared with the MAIN-world bridge (audit #2). Established at
// document_start, before page scripts run, so a page cannot forge a command. Latched
// to the first announcement to resist a page racing the handshake.
let bridgeNonce = null;
window.addEventListener('__victauri_handshake', (event) => {
    if (bridgeNonce === null && event.detail && event.detail.nonce) {
        bridgeNonce = event.detail.nonce;
    }
});
// Ask the MAIN bridge to (re)announce — covers the case where it loaded first.
window.dispatchEvent(new CustomEvent('__victauri_handshake_req'));

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
    if (message.type !== 'victauri_command') return false;

    const responsePromise = new Promise((resolve) => {
        const handler = (event) => {
            if (event.detail && event.detail.id === message.id) {
                window.removeEventListener('__victauri_response', handler);
                resolve(event.detail);
            }
        };
        window.addEventListener('__victauri_response', handler);

        setTimeout(() => {
            window.removeEventListener('__victauri_response', handler);
            resolve({ id: message.id, type: 'error', error: 'Bridge timeout (30s)' });
        }, 30000);
    });

    window.dispatchEvent(new CustomEvent('__victauri_command', {
        detail: { id: message.id, method: message.method, args: message.args, nonce: bridgeNonce }
    }));

    responsePromise.then(sendResponse);
    return true;
});

chrome.runtime.sendMessage({ type: 'content_script_ready', url: location.href });
