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
});
