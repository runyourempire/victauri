// Content script — ISOLATED world
// Relays commands between the service worker and the MAIN world content script.
// Has access to chrome.runtime but NOT to page JS globals.

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
        detail: { id: message.id, method: message.method, args: message.args }
    }));

    responsePromise.then(sendResponse);
    return true;
});

chrome.runtime.sendMessage({ type: 'content_script_ready', url: location.href });
