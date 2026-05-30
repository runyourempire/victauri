# The macOS Wedge — verified, with sources

**Claim:** On macOS, external Tauri automation tools cannot attach to the WKWebView
at all, and the tools that *can* run there (new embedded WebDrivers) are DOM-only.
**Victauri is the sole full-stack option on macOS** — DOM + IPC + Rust backend + DB
+ native, through one MCP interface.

This is the strongest, most defensible part of Victauri's positioning, because it is
*positional* (where the competition structurally can't go), not a capability one
opponent can replicate out-of-process.

## Verified facts (not assertion)

1. **The official Tauri WebDriver path does not run on macOS.** Tauri's own docs:
   > "On desktop, only Windows and Linux are supported due to macOS not having a
   > WKWebView driver tool available."
   Apple ships no WebDriver for WKWebView, so Selenium / WebdriverIO via
   `tauri-driver` cannot test a macOS Tauri app. ([Tauri WebDriver docs](https://v2.tauri.app/develop/tests/webdriver/), [tauri#7068](https://github.com/tauri-apps/tauri/issues/7068))

2. **WebDriver is DOM-only — the whole competitor category.** Tauri's docs describe
   WebDriver as "a standardized interface to interact with web documents" — its scope
   is the webview DOM. It does **not** cover the Rust backend, IPC, or the database.
   ([Tauri WebDriver docs](https://v2.tauri.app/develop/tests/webdriver/))

3. **External CDP/Playwright can't attach to WKWebView either.** WKWebView exposes
   WebKit's own remote inspector, not CDP; Playwright drives its *own* WebKit build,
   not an embedded WKWebView in a third-party app. There is no external-attach path
   on macOS.

4. **Honest nuance (this corrected my first draft):** as of early 2026 there ARE
   community **embedded** WebDriver servers for macOS Tauri — e.g.
   [danielraffel/tauri-webdriver](https://github.com/danielraffel/tauri-webdriver)
   (W3C WebDriver v1 for WKWebView) and
   [Choochmeque/tauri-webdriver](https://github.com/Choochmeque/tauri-webdriver)
   (cross-platform, embedded). They use Victauri's *architecture* (server inside the
   app), so "nothing works on macOS" is no longer true. **But they are WebDriver —
   DOM-only by protocol.** None give IPC / Rust backend / database / native
   introspection. So even where macOS DOM automation now exists, Victauri remains the
   only full-stack option — and the only MCP-native one.

## Victauri's macOS readiness (code audit, this repo)

| Capability | macOS status |
|---|---|
| MCP/REST server, `eval_js`, `dom_snapshot`, `find_elements` | ✅ platform-independent (Tauri JS-bridge injection) |
| `invoke_command` (IPC → Rust), `get_registry`, `verify_state`, `query_db` | ✅ platform-independent (in-process `AppHandle`) |
| `screenshot` | ✅ implemented (`CGWindowListCreateImage`) — but needs Screen-Recording TCC grant at runtime |
| native window handle | ✅ implemented (`ns_view` → `windowNumber`) |
| `get_memory_stats` | ✅ implemented (`task_info` / `MACH_TASK_BASIC_INFO` → `resident_bytes`) |
| child-process enumeration | ✅ implemented (`proc_listchildpids`) |
| **trusted (`isTrusted:true`) input** | ⚠️ **stubbed** on macOS — falls back to synthetic events (CGEvent impl pending) |

**Honest gap:** the workspace compiles and unit-tests green on macOS in CI, but a
*live Tauri app driven by Victauri on real Apple hardware* had not been demonstrated
end-to-end until the CI job below. Native input is the one true feature gap.

## The proof (real Apple hardware, reproducible)

CI job **`macOS Full-Stack Proof`** (`.github/workflows/ci.yml`) launches the demo
Tauri app on a `macos-latest` runner and asserts, live, the five layers no macOS
automation tool can reach together:

1. **webview** — `eval_js` `6*7` → `42` inside the WKWebView
2. **dom** — `dom_snapshot` returns ref handles
3. **ipc → rust backend** — `invoke_command get_counter` returns a real value
4. **backend** — `get_registry` enumerates the command surface
5. **native** — `get_memory_stats` returns real `resident_bytes`

Screenshot and trusted input are deliberately excluded: they need
Screen-Recording / Accessibility TCC grants headless CI can't give. The
same-process introspection that *is* the wedge needs none of that.

**Result: PROVEN GREEN on real Apple hardware** — CI run
[26690831417](https://github.com/runyourempire/victauri/actions/runs/26690831417)
(2026-05-30, `macos-latest`, 2m37s). The step runs under `set -euo pipefail` with a
`fail()` that exits non-zero on any missed assertion, so a green result
deterministically means: the Tauri app launched on macOS, the embedded MCP server
came up, and webview-eval (`6*7→42`) + DOM snapshot + IPC→Rust-backend invoke +
383-style registry enumeration + native `resident_bytes` all succeeded — the four
layers below the glass, live, on the platform where external automation can't reach
the webview at all. Re-runs on every push to `main`.

## The honest one-liner

> On macOS, the blessed Tauri E2E tooling doesn't run, and the new embedded drivers
> see only the DOM. Victauri is the one tool that gives an agent the DOM **and** the
> IPC, the Rust backend, the database, and native state — on the platform where
> everything else stops at the glass (or stops at the door).

Sources: [Tauri WebDriver docs](https://v2.tauri.app/develop/tests/webdriver/) ·
[tauri#7068 (macOS support request)](https://github.com/tauri-apps/tauri/issues/7068) ·
[danielraffel/tauri-webdriver](https://github.com/danielraffel/tauri-webdriver) ·
[Choochmeque/tauri-webdriver](https://github.com/Choochmeque/tauri-webdriver)
