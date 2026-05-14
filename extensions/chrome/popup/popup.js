document.addEventListener('DOMContentLoaded', async () => {
    const hostStatus = document.getElementById('host-status');
    const bridgeStatus = document.getElementById('bridge-status');
    const tabUrl = document.getElementById('tab-url');
    const tabTitle = document.getElementById('tab-title');
    const mcpUrl = document.getElementById('mcp-url');

    // Get active tab info
    const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
    if (tab) {
        tabUrl.textContent = tab.url || '—';
        tabTitle.textContent = tab.title || 'Untitled';
    }

    // Check native host connection
    try {
        const response = await fetch('http://127.0.0.1:7474/health');
        if (response.ok) {
            hostStatus.textContent = 'Connected';
            hostStatus.className = 'status connected';

            document.querySelectorAll('.action-btn').forEach(btn => {
                btn.disabled = false;
            });
        }
    } catch (e) {
        // Host not running
    }

    // Check bridge status
    if (tab && tab.id) {
        try {
            const result = await chrome.tabs.sendMessage(tab.id, {
                type: 'victauri_command',
                id: 'popup-check',
                method: 'getDiagnostics',
                args: {}
            });
            if (result && result.type !== 'error') {
                bridgeStatus.textContent = 'Ready';
                bridgeStatus.className = 'status connected';
            }
        } catch (e) {
            // Bridge not ready on this page
        }
    }

    // Copy MCP URL on click
    mcpUrl.addEventListener('click', () => {
        navigator.clipboard.writeText(mcpUrl.textContent);
        mcpUrl.style.color = '#22c55e';
        setTimeout(() => { mcpUrl.style.color = ''; }, 1000);
    });

    // Action buttons
    document.getElementById('btn-screenshot').addEventListener('click', async () => {
        if (!tab || !tab.id) return;
        try {
            const result = await sendCommand(tab.id, 'screenshot', {});
            if (result) {
                const blob = await (await fetch('data:image/png;base64,' + (result.data || result))).blob();
                const url = URL.createObjectURL(blob);
                chrome.tabs.create({ url });
            }
        } catch (e) {
            console.error('Screenshot failed:', e);
        }
    });

    document.getElementById('btn-a11y').addEventListener('click', async () => {
        if (!tab || !tab.id) return;
        try {
            const result = await sendCommand(tab.id, 'auditAccessibility', {});
            console.log('A11y audit:', result);
        } catch (e) {
            console.error('A11y audit failed:', e);
        }
    });

    document.getElementById('btn-snapshot').addEventListener('click', async () => {
        if (!tab || !tab.id) return;
        try {
            const result = await sendCommand(tab.id, 'snapshot', { format: 'compact' });
            console.log('DOM snapshot:', result);
        } catch (e) {
            console.error('Snapshot failed:', e);
        }
    });
});

async function sendCommand(tabId, method, args) {
    return new Promise((resolve, reject) => {
        chrome.tabs.sendMessage(
            tabId,
            { type: 'victauri_command', id: 'popup-' + Date.now(), method, args },
            (response) => {
                if (chrome.runtime.lastError) {
                    reject(new Error(chrome.runtime.lastError.message));
                } else if (response && response.type === 'error') {
                    reject(new Error(response.error));
                } else {
                    resolve(response);
                }
            }
        );
    });
}
