# Agent-Eval Task Corpus

Reproducible debugging scenarios for the A/B agent-eval (see
`.claude/plans/agent-eval-harness.md`). Target: the **demo-app** running on
`http://127.0.0.1:7373` (`auth_disabled`). Each task: a **setup** (creates the
bug state via Victauri's own tools), the **goal** given to the agent, the
**answer key** (true root cause, for scoring), and **why Agent-B should win**.

Agent-B = full Victauri toolset. Agent-A = browser-only (`eval_js` + `dom_snapshot`
ONLY — no backend/IPC/DB introspection), simulating CDP/Playwright.

REST helper: `curl -s -X POST http://127.0.0.1:7373/api/tools/<tool> -d '<json>'`

---

## T1 — Cross-boundary divergence (UI lies about backend state)
- **Setup:** drive the UI a few times, then corrupt only the DOM:
  `eval_js {"code":"document.querySelector('[data-testid=counter-value]').textContent='99'; return 'set'"}`
  (backend `get_counter` is unchanged; the UI now disagrees with the backend.)
- **Goal:** "The counter shows 99 in the UI. Is the displayed value actually consistent
  with the backend's counter state? If not, explain the discrepancy."
- **Answer key:** DOM shows 99 but `invoke_command get_counter` returns the real (different)
  value → frontend/backend divergence; the DOM is stale/wrong, backend is source of truth.
- **Why B wins:** `verify_state` (frontend_expr vs backend) or `invoke_command get_counter`
  exposes the mismatch in one call. Agent-A sees only "99" in the DOM and has no way to know
  the backend disagrees.

## T2 — Ghost command (frontend calls an unregistered command)
- **Setup:** `eval_js {"code":"window.__TAURI_INTERNALS__.invoke('ghost_secret_cmd',{}).catch(()=>{}); return 'invoked'"}`
  (a command name the backend doesn't register, now present in the IPC log).
- **Goal:** "Is the frontend invoking any IPC command that the backend does not actually
  define/register? Name it."
- **Answer key:** `ghost_secret_cmd` is invoked by the frontend but is not in the registry.
- **Why B wins:** `detect_ghost_commands` returns it directly. Agent-A cannot enumerate the
  backend command registry from the browser.

## T3 — Swallowed IPC error (UI shows nothing, command failed)
- **Setup:** `fault {"action":"inject","command":"submit_contact","fault_type":"error","error_message":"INJECTED_DB_ERROR"}`
  then submit the contact form via the UI (fill contact-* fields, click contact-submit).
- **Goal:** "The contact form submit appears to do nothing. Diagnose what actually happens
  when the user submits."
- **Answer key:** `submit_contact` returns an injected error (`INJECTED_DB_ERROR`); the IPC
  call failed but the UI swallowed it. (`fault list` shows the injected rule.)
- **Why B wins:** `logs ipc` / `check_ipc_integrity` shows the errored call; `fault list` shows
  the cause. Agent-A sees an unchanged DOM and cannot inspect the IPC layer.
- **Cleanup:** `fault {"action":"clear_all"}`

## T4 — Intermittent flake (diagnose a non-deterministic failure)
- **Setup:** `fault {"action":"inject","command":"increment","fault_type":"error","error_message":"FLAKE","max_triggers":2}`
- **Goal:** "Increment sometimes works and sometimes doesn't. Diagnose the flakiness and its
  exact cause."
- **Answer key:** an injected fault makes `increment` error its first 2 invocations
  (`max_triggers:2`), then succeed → looks flaky.
- **Why B wins:** `fault list` shows the rule + trigger count; `logs ipc` shows the errored
  vs successful calls. Agent-A cannot see the IPC errors or reproduce-on-demand.
- **Cleanup:** `fault {"action":"clear_all"}`

## T5 — Backend-only state bug (UI looks fine, backend is wrong)
- **Setup:** change a setting via the UI (theme-select), then have the backend hold a different
  value. (Repro: `invoke_command update_setting` with one value, then `eval_js` set the
  select's displayed value to a different one — UI and backend now disagree on the setting.)
- **Goal:** "The theme setting shows 'dark' in the UI. Confirm what the backend has actually
  persisted for the theme, and whether they agree."
- **Answer key:** UI select shows 'dark' but `get_settings` (backend) holds the other value →
  divergence; backend is the truth.
- **Why B wins:** `invoke_command get_settings` (or `query_db`) reads backend truth. Agent-A
  can only read the DOM select.

## T6 — CONTROL: pure-DOM bug (browser-only SHOULD succeed) — fairness check
- **Setup:** `eval_js {"code":"document.querySelector('[data-testid=reset-btn]').style.pointerEvents='none'; return 'disabled'"}`
- **Goal:** "The reset button doesn't respond to clicks. Why?"
- **Answer key:** `reset-btn` has `pointer-events: none` (a CSS/DOM issue).
- **Why this exists:** the bug is entirely in the DOM/CSS, so Agent-A (browser-only) should
  diagnose it fine. If Victauri doesn't also win/tie here, the harness is rigged — this keeps
  the experiment honest.
- **Cleanup:** `eval_js {"code":"document.querySelector('[data-testid=reset-btn]').style.pointerEvents=''; return 'restored'"}`

## T7 — Miscalibrated sweep animation (motion is invisible to screenshots)
- **Setup:** trigger the demo-app's deliberately-broken slide-in:
  `eval_js {"code":"document.getElementById('sweep-btn').click(); return 'played'"}`
  (`.sweep-toast` runs `sweepBroken`: `translateX(420px)`→`translateX(-48px)` over 1200ms with an
  overshooting `cubic-bezier(0.5,-0.6,0.9,1.4)`. Intended: settle flush at `translateX(0)` over
  ~300ms, gentle ease-out.)
- **Goal:** "The notification slide-in looks wrong — it overshoots and doesn't land where it
  should. Quantify exactly what the animation does (start/end position, duration, easing) and
  state precisely how it diverges from a clean 300ms ease-out that settles at the edge."
- **Answer key:** duration is 1200ms (4× too long); the easing's negative control point makes it
  travel *backwards* off-screen early (tx 420→~473 before reversing); it ends at tx **−48** (48px
  past the target, which should be 0). Three concrete defects: duration, easing overshoot, end offset.
- **Why B wins:** `animation list` returns the declared duration/easing/keyframes in one call;
  `animation scrub` returns the exact position-vs-progress curve **and** a single deterministic,
  jank-free filmstrip image of the whole arc; `animation sample` returns the measured real-time
  curve. Agent-A (browser-only) can only `eval_js`/`dom_snapshot`/`screenshot`: a screenshot is a
  single aliased instant (motion blur, race) and **cannot** rasterize a frozen seeked frame, so it
  cannot produce the curve or the filmstrip. (Honesty caveat: a *very* clever Agent-A could hand-roll
  `getAnimations()`+a rAF sampler through `eval_js`; B's win here is one-call ergonomics, reliability,
  and the native-capture filmstrip — which is genuinely B-only, since JS cannot screenshot the webview.)
- **Cleanup:** none needed (re-triggerable; leaves no persistent state).

---

## Scoring rubric (per agent, per task)
- `solved` — did it state the TRUE root cause (matches answer key)?
- `tool_calls` — count.
- `reverted` — did it ask for CDP/Playwright, give up, or claim it "can't tell"? (headline metric)
- `wrong_tool` / `confusion` — notes where tool descriptions/returns misled it.
- For T6, expect A and B to BOTH solve (fairness).

## Expected pattern (the hypothesis being tested)
T1–T5: Agent-B solves; Agent-A cannot (structurally blind to backend/IPC) → reverts or guesses.
T6: both solve. If the data shows this, it's direct evidence that full-stack visibility makes
agents materially better at debugging Tauri apps — the central claim.
