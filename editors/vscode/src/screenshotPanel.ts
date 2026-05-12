import * as vscode from "vscode";
import { VictauriClient } from "./client";

export class ScreenshotPanel {
  private static instance: ScreenshotPanel | undefined;
  private readonly panel: vscode.WebviewPanel;

  static show(context: vscode.ExtensionContext, client: VictauriClient): void {
    if (ScreenshotPanel.instance) {
      ScreenshotPanel.instance.panel.reveal();
      ScreenshotPanel.instance.refresh(client);
      return;
    }
    ScreenshotPanel.instance = new ScreenshotPanel(context, client);
  }

  private constructor(
    context: vscode.ExtensionContext,
    private readonly client: VictauriClient
  ) {
    this.panel = vscode.window.createWebviewPanel(
      "victauri.screenshot",
      "Victauri Screenshot",
      vscode.ViewColumn.Beside,
      { enableScripts: true, retainContextWhenHidden: true }
    );

    this.panel.onDidDispose(() => {
      ScreenshotPanel.instance = undefined;
    });

    this.panel.webview.onDidReceiveMessage(async (msg) => {
      if (msg.command === "refresh") {
        await this.refresh(client);
      } else if (msg.command === "save") {
        await this.save(msg.data);
      }
    });

    this.refresh(client);
  }

  private async refresh(client: VictauriClient): Promise<void> {
    try {
      const data = await client.screenshot();
      if (data) {
        this.panel.webview.html = this.getHtml(data);
      }
    } catch (e) {
      this.panel.webview.html = this.getErrorHtml(String(e));
    }
  }

  private async save(base64Data: string): Promise<void> {
    const uri = await vscode.window.showSaveDialog({
      defaultUri: vscode.Uri.file(`victauri-screenshot-${Date.now()}.png`),
      filters: { Images: ["png"] },
    });
    if (uri) {
      const buf = Buffer.from(base64Data, "base64");
      await vscode.workspace.fs.writeFile(uri, buf);
      vscode.window.showInformationMessage(
        `Screenshot saved to ${uri.fsPath}`
      );
    }
  }

  private getHtml(base64Data: string): string {
    return `<!DOCTYPE html>
<html>
<head>
<style>
  body { margin: 0; padding: 16px; background: var(--vscode-editor-background); display: flex; flex-direction: column; align-items: center; }
  img { max-width: 100%; border: 1px solid var(--vscode-panel-border); border-radius: 4px; }
  .toolbar { margin-bottom: 12px; display: flex; gap: 8px; }
  button { background: var(--vscode-button-background); color: var(--vscode-button-foreground); border: none; padding: 6px 14px; cursor: pointer; border-radius: 2px; font-size: 13px; }
  button:hover { background: var(--vscode-button-hoverBackground); }
  .meta { color: var(--vscode-descriptionForeground); font-size: 12px; margin-top: 8px; font-family: var(--vscode-font-family); }
</style>
</head>
<body>
  <div class="toolbar">
    <button onclick="vscode.postMessage({command:'refresh'})">Refresh</button>
    <button onclick="vscode.postMessage({command:'save',data:'${base64Data}'})">Save As...</button>
  </div>
  <img src="data:image/png;base64,${base64Data}" />
  <div class="meta">Captured at ${new Date().toLocaleTimeString()}</div>
  <script>const vscode = acquireVsCodeApi();</script>
</body>
</html>`;
  }

  private getErrorHtml(error: string): string {
    return `<!DOCTYPE html>
<html><body style="padding:16px;color:var(--vscode-errorForeground);font-family:var(--vscode-font-family);">
  <p>Screenshot failed: ${error}</p>
  <button onclick="vscode.postMessage({command:'refresh'})" style="background:var(--vscode-button-background);color:var(--vscode-button-foreground);border:none;padding:6px 14px;cursor:pointer;">Retry</button>
  <script>const vscode = acquireVsCodeApi();</script>
</body></html>`;
  }
}
