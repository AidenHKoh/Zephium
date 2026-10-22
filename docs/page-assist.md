# Ask panel (page assist): Rust state and reader handoff

Status as of 2026-10-22, branch `feat/work-production`.

## What exists

| Piece | Where | State |
|---|---|---|
| IPC contract | `crates/zephium-ipc/src/page_assist.rs`, `desktop/src/page_assist.rs` | Complete; bindings generated. |
| Pure core | `crates/zephium-page-assist` | Prompt layout (fixed system text, fenced untrusted page and selection blocks, page-first layout for prompt caching), line-protocol parser, citation numbering with verbatim highlight text, pacer, refusal policy, per-tab threads (24 turns, 64 tabs LRU, same-site vs cross-site context), summary cache, cost display, action grammar and 8-step plan, up-front "Run in Work" detection, CSS validator. Boundary: `cargo xtask check-page-assist-boundary`. |
| Service | `crates/zephium-work-composition/src/page_assist/` | Turn lifecycle, model call through `resolve_entry_for(…, Page, Some(WorkCallSource::Page{tab, turn}))`, streaming parse and pacing, cancellation (Stop, navigation, tab close, AI off), fault mapping with retry hints, cost (hosted settled later, BYOK priced or estimated, local free), steps under a lease with confirm for Commit and pause on human input, page styles applied only after Apply. 20 tests including a loopback SSE provider. |
| Page-style caps | `page_assist::MAX_SITE_STYLES` (8), `MAX_SITE_CSS_BYTES` (48 KiB) | Past either cap a style turn ends with `style.state = full` before any request; storage refuses a row over 60 KiB without writing. |
| Desktop wiring | `desktop/src/page_assist.rs`, hooks in `desktop/src/lib.rs` and `desktop/src/account.rs` | Commands call the service; tabs followed from `Projection::Items`/`Projection::Tab`; AI off ends every thread; styles persist per profile and site in app settings (validated again on load); hosted charges reach the answer when the receipt settles. |
| Page-style slot | `crates/zephium-engine/src/host/content_styles.rs`, `content_style.js` | `Engine::set_page_style(profile, site, css)`; delivered as the `page-style` adopted sheet after `personal`, matched by registrable site including subdomains. |
| Human-wall survey | `platform/macos/semantic_runtime.rs` | Runs as the closed `H1:survey` pull of the installed runtime; no `evaluateJavaScript` remains in that file. |

## What is not done

1. **The live Browse-tab reader (macOS and Windows).** `DesktopPage::supported()` returns false, so every tab reports `unsupported` and no request is ever sent. Everything above it is ready for it.
2. **Highlight without the find bar** (`PagePort::highlight`) — part of the reader.
3. **Light actions** (`PagePort::resolve` / `act`) — part of the reader; `resolve` must judge effects with `open_objective::site_work::classify` on the read's `SemanticSnapshot`.
4. **"Run in Work"** is wired: `desktop/src/page_assist.rs` (`handoff`) creates a Work environment in the tab's Space, adds the tab (`WorkEnvironmentReference::Browser`) and the new objective, and begins a run through `work_product::admit_run` with the panel's own grant and limits (pinned against `frame/src/domain/work/work-model.ts` by a test). A tab not yet listed in a Space cannot be handed off.
5. **Tab close** is read from the shell's Items projection: a tab its profile and Space listed before and no longer list ends its thread and cancels its turn. A tab moved to another Space is treated the same way.
6. **Security fixture** for the survey change (a page calling `window.open()` right after our pull answer must stay blocked) is not written; it needs a live WebKit view in a probe.

## Reader: chosen design

Founder bar: lighter than protection-off on CPU and RAM; no user activation from automation; Rust is the only source of program bytes.

### macOS — evaluate first: bootstrap-only

- One **~1 KB document-start `WKUserScript`** per Browse view's user content controller, main frame only, in a **private `WKContentWorld`** created for the view. It holds exactly one pending reply pull on a dedicated reply handler (for example `zephiumPageAssistBootV1`) and does nothing else.
- When the person asks, Rust answers that pull with the fixed runtime program (`SEMANTIC_RUNTIME_PROGRAM.source()`, the pinned constant, never page-derived) and the bootstrap instantiates it inside the private world (for example with `Function`). The runtime then speaks the existing closed channel (`P1`/`R1:`/`A1`/`S1`) on its own reply handler in the same world.
- **Verify before building on it**, with loopback fixtures in a probe (never on the founder's screen):
  - a page served with `Content-Security-Policy: script-src 'self'` (no `'unsafe-eval'`): instantiation in the private world still works;
  - a page that calls `window.open()` right after our pull answer and polls `navigator.userActivation.isActive`: stays blocked and inactive;
  - the page world cannot observe the private world's globals or the handler.
- **If WebKit refuses** instantiation under page CSP: fall back to installing the unmodified runtime wrapped in a never-called function at document start (lazy pre-parse only), and measure first: A/B over about ten real pages for load time and WebContent footprint. Accept only at ≤ ~1 ms main-thread and ≤ ~250 KB per page; otherwise report the numbers.
- Either way: never install on sensitive sites (`zephium_app::work_sites::is_sensitive`), in private profiles, or with AI off; remove the script and handlers from the controller when AI is switched off. Tabs whose document predates installation report `loading` until their next navigation.
- The owned-view registration (`AgentSemanticRuntimeRegistration`) cannot be reused: it removes all user scripts and attests exactly one. Build a sibling, for example `platform/macos/browse_reader.rs`, reusing `AgentSemanticRuntimeController`'s state machine through a narrow constructor export from `semantic_runtime.rs` (pinned file: keep the edit minimal, and keep `cargo xtask check-agentic-probe-boundary` green).
- Epoch binding: begin/commit document loads from the Browse navigation observer (`host/navigation.rs`, `NavigationEpochTracker`), so a read or step answered for an older document is discarded; the desktop `TabFacts.epoch` must come from the same committed epoch rather than the URL-change counter it uses today.
- Teardown: navigation (begin load invalidates), tab close (retire), idle (stop the runtime's pull after a few minutes without asks; the bootstrap pull remains), AI off (remove).

### Windows — on demand, no document-start piece

Through the closed CDP protocol (`platform/agent_semantic_cdp_protocol.rs`): on first ask, create a fresh, unpredictable isolated world for the current document (`grantUniveralAccess: false`), join its execution-context event, install `cdp_source()` with `Runtime.callFunctionOn` (`userGesture: false`), and invoke reads through the same fixed functions. Coexist with extensions and the person's DevTools: one CDP request at a time, detach cleanly, and treat a lost context as `PageChanged`.

### Reader output

`Readout { epoch, title, passages }` (`page_assist/port.rs`). Build it from the runtime's `SemanticSnapshot` via `read_semantic_observation` with `SemanticReadSensitivityLimit::PublicOnly` and `SemanticReadBudget::STANDARD`: one `Passage` per read fragment, `id` = the fragment's `rN`, `verbatim` only when `verbatim_text()` is `Some`, `value` never for protected fields (the service also withholds protected kinds and secret-shaped values). Selection and highlight need two more fixed recipes in the runtime (select-text read; scroll-to-and-mark a verbatim quote without the find bar), each a closed invocation like `H1:survey`.

## Tests still needed

- Reader probe on loopback fixtures (`zephium-terra-macos-probe --loopback-site` pattern): read, CSP fixture, activation fixture, epoch discard after navigation, teardown on close and AI off.
- Highlight fixture: the quote is scrolled to and marked, the find bar never opens.
- Step fixtures: a Commit control waits for confirm; a password field is refused; human input pauses.
- Windows: the same reads through CDP with an extension's content script and DevTools attached.
