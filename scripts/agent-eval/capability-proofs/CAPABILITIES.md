# Capability Proofs — what Victauri can do that a browser tool structurally cannot

**Type:** deterministic capability proofs (binary facts, reproducible) — NOT a
stochastic outcome A/B. No LLM judge, no statistics needed: either the tool reads
the database or it doesn't.

**Target:** **4DA** — a real, shipping Tauri app — running live with `victauri-plugin`
(MCP/REST on `127.0.0.1:7374`, 383 backend commands, auth disabled for local test).
**Real production database:** `D:\4DA\data\4da.db` — **322 MB**, 82,494 pages, WAL,
100+ tables. Captured 2026-05-31. All output below is verbatim from the live app.

The control ("browser-only") simulates CDP/Playwright: DOM + `eval_js` in the
webview, no Victauri backend/DB tools, no `window.__VICTAURI__`.

---

## Proof 1 — Read the real 322 MB SQLite database

`introspect db_health`:
```json
{"database":"../data/4da.db","db_size_mb":322.24,"integrity_check":"ok",
 "journal_mode":"wal","page_count":82494,"page_size":4096,
 "tables":[{"name":"active_topics","row_count":3323},{"name":"ai_usage","row_count":1131}, ...100+ tables]}
```

A real analytical query (one statement spanning three tables):
```sql
SELECT COUNT(*) AS active_topics,
       (SELECT COUNT(*) FROM ai_usage)  AS ai_calls,
       (SELECT COUNT(*) FROM audit_log) AS audit_rows
FROM active_topics;
```
```json
{"rows":[{"active_topics":3323,"ai_calls":1131,"audit_rows":0}]}
```

## Proof 2 — Read backend-internal data that the UI never renders

`accuracy_history` is an internal scoring-metrics table — not shown on any screen,
not exposed by any "getter" IPC command. Victauri reads it directly:
```json
{"columns":["id","period","total_scored","total_relevant","user_confirmed","user_rejected","accuracy_pct","created_at"],
 "rows":[{"id":5,"period":"2026-W21","total_scored":125610,"total_relevant":21860,"accuracy_pct":0.1740,"created_at":"2026-05-26 14:43:50"}, ...]}
```
An agent debugging "why is relevance accuracy low?" gets the ground truth (17.4%,
125,610 items scored) in one call. This data is **below the entire IPC/UI surface** —
a browser tool can't reach it even by invoking every command the app exposes.

## Proof 3 — The browser-only path is structurally blind to the DB

Best-effort `eval_js` probe for any in-webview path to the SQLite file
(simulating what CDP/Playwright could try):
```json
{"sqlite_in_window": false, "has_require": false, "has_fs_api": false,
 "indexedDB_dbs_note": "only webview-origin IndexedDB, not the Rust SQLite file"}
```
The webview JS context has **no filesystem, no SQLite, no `require`**. The 322 MB
Rust-side database is unreachable from the browser layer by construction — not a
missing feature, a hard boundary.

## Proof 4 — Enumerate the full backend command surface

`get_registry` returns all **383** registered Tauri commands
(`ace_auto_discover`, …). `eval_js` can *invoke* a command it already knows the
name of (via `window.__TAURI_INTERNALS__.invoke`), but there is **no web API to
list registered Tauri commands** — you can't discover what you can't already name.

---

## What this does and does not prove

**Proves (deterministic):** Victauri, running in-process, reads the real database
(including tables no UI/IPC exposes) and enumerates the backend command surface —
capabilities a browser-attached tool (CDP/Playwright) cannot have, by construction.
This is the unfakeable moat.

**Does NOT prove (needs the rigorous outcome study):** that an agent *debugs better*
overall with Victauri. That is a stochastic claim requiring real apps, real bugs
from git history, n≥5 per cell, a blind judge, and a steelmanned opponent — see
`../RESULTS.md` (pilot, bug-finding only) and the methodology in
`.claude/plans/`.

**Honest boundary (from the same testing):** the edge is the *same-process Rust
side* — DB, registry, native state, memory. It is **not** DOM control (Playwright's
equal), **not** backend reads via JS `invoke` (a browser tool can do those), and
**not** IPC *control*/fault injection (a CDP tool can do more there than Victauri).
Lead with these DB/introspection capability proofs, not control features.

## Reproduce
With a Victauri-instrumented app on `<PORT>` (4DA shown on 7374):
```bash
B=http://127.0.0.1:7374/api/tools
curl -s -X POST $B/introspect -d '{"action":"db_health"}'
curl -s -X POST $B/query_db   -d '{"query":"SELECT name FROM sqlite_master WHERE type=\"table\""}'
curl -s -X POST $B/get_registry -d '{}'
curl -s -X POST $B/eval_js -d '{"code":"return typeof window.sqlite"}'   # -> "undefined"
```
