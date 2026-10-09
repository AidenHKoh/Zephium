# Work execution foundation

`AgentWorkController` is a production-feature implementation of the existing
`AgentRuntimeController`, not a diagnostic controller or a second automation
stack. It owns `AgentBrowserSession` and drives the real `AgentRuntimeBrowser`
port. This is an executable foundation, not yet a user-facing Work product or
a production qualification claim.

## Application boundary

The trusted application admits an `AgentWorkRunInput`: one approved manifest
root, exact owned-context/profile/initial-target assignment, task objective,
catalog model, stable identifiers, clock and absolute deadline. An
`AgentWorkTask` supplies a trusted task-level predicate, independent effect
classification and exact account attestation. Neither a model response nor
hostile page text may implement these authorities. Model text and action counts
cannot terminate a task successfully.

For independently classified local form preparation, the production
[`AgentWorkFormTask`](agent-work-forms.md) supplies bounded trusted field/value
phases and exact fresh-target assessment without a bespoke Rust predicate.
It is not an automatic effect classifier for arbitrary forms.

The optional [application admission boundary](agent-work-persistence.md) now
owns durable admission and the complete runtime lifecycle. The composition root
prepares `PreparedAgentWork`, attaches the same Store's journal port to the
shell, and submits through `AgentWorkApplicationHandle`; native creation waits
for both durable admission acknowledgements. No UI or default-desktop agent
dependency is enabled by this seam.

For direct trusted compositions, construct `AgentWorkController::try_new` with the transport configuration,
move-only credential and existing `AgentAuditPort`. Move it into
`PendingAgentRuntime::spawn_suspended_with_controller`, bind the actual
`EngineHost::take_agent_browser_port`, and retain the existing runtime handle,
completion handle and lifecycle owner. Only explicit `start_run` begins native
or provider work. `AgentWorkHandle` exposes bounded content-free events and the
single move-only terminal/recovery outcome; it exposes no page/native internals.

The caller must keep pumping the engine's native dispatcher during execution
and shutdown. The selected browser profile must already have its authoritative
content policy applied by the ordinary profile/policy owner before constructing
a Work page. Application admission waits for that existing readiness under the
original deadline; inventing a profile or installing a task-specific fallback
policy is not a repair. An uninstalled policy is a refusal, not permission to
bypass profile security. The [composition binding](agent-work-composition.md)
preserves the actor-selected session's profile and persistence class.

## Ownership and bounds

The actor uses the original supervisor, policy, audit ledger and four metric
reducers. Its original context registry, profile leases, cookie-transfer and
screenshot coordinators join the session's original native execution and
settlement coordinators for shutdown. No replacement empty resource cohort
may manufacture a clean proof.

- One run, one root and one independently managed extension-free owned page.
- At most eight model turns and eight one-action turns; locate is bounded by
  the existing semantic matcher. No action or provider request is blindly retried.
- Action tasks advertise only locate and snapshot-verifiable act. A trusted
  `AgentWorkExtractionTask` instead advertises only initial-scope extraction;
  its [bounded result](agent-work-results.md) remains explicitly model-mapped.
  A trusted task can explicitly combine these existing tools: its frozen schema
  and mode require fresh `ReadyForExtraction` before mapping and refuse further
  actions afterward. All phases share the same eight-call ceiling and original
  policy, native and result-publication owners.
  Immediate and
  mutation-quiet settlement use the core's bounded wake schedule before one
  adjacent fresh observation. Navigation/dialog/scroll adapters are absent.
- The absolute deadline is bounded by the approved root expiry and ten minutes.
  Explicit read-only `NotReady` receipts permit at most 64 readiness checks at
  50 ms intervals, with fresh invocation IDs but unchanged snapshot generation.
  Other failures do not trigger observation, action or model replay.
- Product progress has a preallocated 64-event FIFO with stable run/sequence
  correlation. Overflow is sticky backpressure; no overwrite or extra worker.
  The move-only terminal slot is separate, so progress pressure cannot discard
  an already-proven completion.
- Deferred native callbacks are bounded by the existing runtime mailbox's
  maximum terminal plus signal capacity. Overflow remains unclean recovery.

The existing single runtime worker now enables its I/O reactor as well as its
timer. Construction of an unused controller starts no worker, timer, request or
native page. The runtime still has no provider HTTP dependency. The default
desktop graph and Browse scheduling are unchanged. This is structural evidence,
not measured battery, RAM or CPU qualification.

## Stops, audit and terminal truth

The current Work capture admits its exact main frame only. Embedded frames are
retained as explicit `Unsupported(PolicyBlocked)` boundaries before observation
assembly, including in a requested subtree. They do not erase usable main-page
content or silently disappear, and they authorize no child capture, origin
inference, native call or action. Read/extraction projection marks a parent with
unobserved child boundaries `source_incomplete` even when the native main-frame
snapshot itself completed. The task must be satisfied by the actual cited
main-page evidence; embedded content remains outside this controller's scope.

Trusted extraction schemas can also narrow mapping evidence to a frozen closed
set of semantic source roles. This filters the already-authorized bounded
capture after privacy checks, not the native traversal or initial inspection.
It introduces no model tool option, new account/origin/frame authority, retry or
larger ceiling. `role_selection` distinguishes intentionally excluded readable
fields from `source_incomplete`; the exact selection is schema/read/citation
bound. The default remains all roles. See [results](agent-work-results.md).

Account evidence is sampled through the trusted task before each provider or
effect admission, including locate/read continuations, verified-action diffs
and extraction mapping. The actor owns that outer inspection loop; several
model calls cannot hide behind one startup sample. It checks sticky controls
before and after the bounded, synchronous account adapter. This adds no worker,
timer, native capture, model request or account-discovery mechanism.

The session freezes the admitted account and complete context/document join.
New samples must be fresh under the unchanged 30-second core policy limit,
non-regressing and non-future. An attestation identity cannot be rewritten or
replayed, and the fixed inventory is bounded by the model/effect ceilings.
An unchanged cached sample keeps its original age. Refresh over an original
provider/effect reservation refuses without replacing that owner. Refusal is
sticky and new identity requires fresh authorized admission, even when both
accounts appear in a manifest.

The built-in form/extraction predicates contain a constructor-supplied account
scope, not a live sign-in detector. They now preserve their first account sample
instead of minting newer timestamps. Without an independently sourced account
adapter they still fail closed when that sample expires. A trusted task may
delegate its predicate to these types and supply fresh account facts through
`attest_account`; neither page/model claims nor a timestamp update are evidence.
Native authenticated-account discovery and monitoring remain unimplemented.

Cancellation, human takeover, policy revocation and suspension requests use the
existing sealed control lane with a first-wins typed reason. Shutdown remains
the stronger lifecycle event. Renderer/navigation loss, mailbox faults,
deadlines and provider/native refusals fail closed. Rust invalidates old joins
and observations before native revocation. Already-dispatched callbacks remain
owned even when a provider future is aborted or native automation is revoked.

Recovery schedules at most one separately tracked lifecycle close after
revocation; it does not wait for a lost observation or cancellation callback to
exhaust cleanup time. Exact close acknowledgement may release the physical
context/profile owner without erasing any other callback/accounting debt.
Callback loss, close refusal and timeout retain that debt in `AgentWorkRecovery`.
Default recovery draining is bounded to one second, or the existing lifecycle
shutdown deadline. Closing a page is not evidence that a native action succeeded.

Success requires the trusted task predicate, independently verified and charged
effects, exact provider accounting/drain, durable content-free audit delivery,
the original native-resource shutdown proof, metric/policy closure, and an exact
runtime terminal claim with no mailbox debt. Runtime lifecycle Clean additionally
requires normal controller return and successful worker join. A business-only
commit, nonzero native audit, missing provider proof or lost audit callback can
never establish Clean.

Task success is separate from resource cleanliness. The actor can now publish
`ClosedUnsuccessfully` after a fully accounted provider/task refusal or stop,
using those same original resource, metric, policy, audit and runtime proofs.
The exact typed cause remains visible and no extraction result is returned.
The application publishes distinct immutable `Failed` or `Cancelled` only
after the original clean lifecycle join and exact durable terminal ACK. Neither
fact grants another execution or reopens the sealed native port.

This path consumes only exact read-only and successful revocation callbacks;
foreign, duplicate, refused and lost receipts remain recovery debt. It shares
the existing cleanup deadline, including any shorter shutdown deadline, and
never retries an action. A retained native action/accounting owner,
audit refusal, or a previously started terminal-resource close cannot be
reclassified as clean. Failure during an already-started success close remains
recovery even when the page has physically closed.

An exact never-dispatched policy `NeedsHuman` proposal is separately typed and
retained through original unsuccessful closure, then discarded without replay.
It keeps the original supervisor token for drain instead of yielding it away.
The application can then require [explicit durable review and fresh admission](agent-work-review.md).
Other proposal/admission failures remain conservative recovery owners.

Recovery retains the sealed session, policy, journal, native owners and retained
callbacks without credentials or objective data. It deliberately grants no
replay permission. The application may now explicitly reconcile the original
retained audit ledger and an exact uncertain durable write. Failure-path audit
events that have not been acknowledged remain debt, not claimed durability.
Persisted recovery facts classify interruption after restart; they do not
restore live executable state or fabricate a clean controller. Approval review
requires fresh admission and never executes an old proposal.

## Working as you

A run works on a site in the person's own browser session. The lead's
`browse { start, goal, records }` becomes a page task: one Work-owned page on
the start URL's site (its registrable domain), driven by the page agent
(`AgentWorkDiscoveryTask` with the site-session scope and `SiteWorkPolicy`,
at most 60 actions, 40 model calls, 600k model tokens and 9 minutes, 8 of its
own working time after a hold; time held for the person does not count). Its
money is the token ceiling at the page model's highest input rate plus a
2k-token reply per call, never under $0.60 and always inside the run's
remaining share: about $0.60 on GPT-6 Luna, $1.16 on Haiku 4.5 and $4.64 on
Opus 5.5 before the run and part ceilings. A decision whose money covers only
itself and one more call of its size becomes the last one, narrowed to
extract, so the terminal mapping call can still be reserved. The document gate follows the site's own
redirects and same-document routes and cancels page-initiated and cross-site
loads without losing the page. A same-site load a step starts (a search or
filter form, a script navigation) is cancelled and followed as the page
agent's own navigation; a same-site form POST passes only for an approved
commitment. A clicked link whose click changes nothing (a new-window link, a
covered or script-only one) is followed to its observed destination the same
way. After any navigation the arrival gets up to four bounded looks (4 s) to
fill in a loading shell before the next model call.

Page-agent requests keep an append-only prefix for provider prompt caches:
instructions, tools, objective and the first observation, then the turns;
the per-call host facts (act targets, verified-action history, decision and
action budgets) trail the newest turn. Past 48 KiB of transcript the oldest
captures retire to a stub in one batch down to 24 KiB, so the prefix stays
stable for several calls instead of changing on each.

Before a run first works on a site where the profile holds cookies (a
presence fact, never their contents), Rust reads the whole start page before
any model call; a signed-out page goes on without a question, otherwise it
asks once: Allow (this run), Always
for the site (kept per profile, `WorkRequest::SiteAccess`), or Not now (the
site is worked privately). Never and the closed list of sensitive sites
(banks, password managers, health and tax portals; Ask at most) keep the
session closed. A page without the person's session opens in Work's own
website data: one persistent store per profile, apart from the profile's own,
that remembers solved bot checks, consent choices and site preferences across
pages and runs (see `docs/agentic-browsing.md` 4.3; Settings → Work clears
it). A private run (`WorkAgentGrantV1.private`) uses the run's own empty
storage everywhere instead. A plain `read` uses the session only on a site the
run already works on as the person.

A public `read` cites the exact address it read, so a redirect never moves
it. When the page's server (a 3xx) or its own script before the page is ready
sends the first load elsewhere, the native gate refuses that load like any
other and keeps the destination only if a new public read could be admitted
there (safe public HTTPS, the read's own navigation checks). The read fails
as `Redirected` (not a lost page), its step says `The page redirects to …`,
and nothing on it ran. On the same site (its registrable domain, as
`same_work_site` defines it) the driver opens one fresh read of the
destination as its own step, citing that address, within what the first read
left of its share; the lead's hands admit it exactly as a `read` the lead
asked for (the same address checks and, in a private run, the person's
answer). A chain follows at most two redirects and never an address it read;
a cross-site redirect, or one past the bound, goes back to the lead as
`Read … if it is what you need`, with the address admitted for a later
`read`. A lead `read` of a plain `http://` address reads its `https://`
address: Work never loads plain HTTP.

The page agent reads, navigates, searches, filters and fills drafts. A step
Rust reads as committing (send, post, pay, book, delete, share, save, submit a
non-search form, Enter in a composer, an edit that saves as it types), or one
the page agent declares so, is held: the page stays as it is and the run
records a `Confirm` step (headline, action, the exact text, facts from the
page's own region, the page's frame, other sites the text quotes). The person
decides through `ApproveStep`; an autosaving edit also offers "Allow edits on
this site for this run" (`for_run`). An approval runs once, and only while the
step and its preview are unchanged; a changed page asks again, a decline is
never retried. The step's status is its receipt. A step read as harmless whose
page then says it committed something stops the page task. Password and
secret fields are never typed into. A sign-in wall (a password field or a
sign-in form, read by Rust or raised by the page agent) holds the page for the
person; the page continues by itself once their navigation settles back on the
site, off any sign-in path (two-factor, OTP and verification paths count as
signing in). A visible one-time-code, card-number or security-code field holds
the page the same way: as a sign-in step, or as a payment for the person when
the page reads as a checkout. A visible challenge widget in an embedded frame
(hCaptcha, Turnstile, reCAPTCHA's checkbox, read only by the frame's title)
holds it as a human check, and a visible card-field frame from a payment
provider (Stripe, Braintree, Adyen and the like, read only by the frame's
title) holds it as a payment for the person. Rust reads these walls at the start, after every
verified step and after every navigation or followed load, before any model
call is spent on a page the agent cannot pass. A page load the person finishes on the site in an
ordinary tab (a count, never its URL) wakes a held page, which starts over
once in the fresh session. Facts from the person's pages never ride a search
query, provider retention never applies to them, and diagnostics carry only
the page kind and its path class. Origin grants, their page budget and the
single-field update are retired; stored runs that used them still load.

## Web search

A lead run searches the way the person does: `web_search` opens the results
page of the engine set for the address bar (`search.engine`, or the custom
`search.custom-url`) for the query, as if it were typed there, and Rust reads
the results off the page. The navigation policy's sensitive-input guard
decides whether a query may leave at all; a query it refuses is sent nowhere.
The lead's freshness becomes the engine's own parameter where one exists
(Google `tbs`, DuckDuckGo `df`, Bing `filters`, Brave `tf`).

The page opens as a step of the run, beside the batch's other searches, in
Work's own website data: nothing from the person's tabs, but the engine's
consent choice from earlier runs is remembered, so an EU consent wall is met
once rather than on every search. A private run's results page opens in a
fresh ephemeral store of its own. Typed decisions are off, and the results policy takes the
page's whole first look, presses nothing and ends there, so no model sees it
and nothing is charged but the step. The reader
(`zephium_agentic::read_search_page`) keeps up to eight results in the
engine's order, each a title link's destination with Google's `/url?q=`,
Bing's `/ck/a?u=a1…` and DuckDuckGo's `/l/?uddg=` redirects unwrapped, its
title and the engine's own snippet. Ads, the engine's own links, non-web
links and repeats of the same page are left out. Built-in engines read their
own markup (Brave's titles are text inside the link); a custom engine reads
headed links in its main content. The evidence names the `browser` provider
with the engine as its model, carries those results with their snippets and
no answer, and costs no tokens; the lead sees a ranked list with source keys
and reads the pages it picks.

Before that first look the results page settles: its document policy
(`PublicResultsPage`) follows the engine's own same-site redirects and script
reloads (Google answers a fresh client with a page that reloads `/search`) in
place, up to six loads, each preparing the page's semantic runtime for the
new document; a cross-site load is refused. A results page is read by Rust
and never cited by its own address, so its settled document stands for it.

A page with no results says why: a challenge or unusual-traffic page, a
consent page, the engine's own "no results", or a page it cannot read. Rust
never answers a challenge or a consent page. The search then falls back to
the provider search behind it, the chosen models' own search or else the
OpenAI public search, at most `BROWSER_SEARCH_FALLBACKS` (three) times a run;
past that, or with none, the step fails as a search that could not be sent.
The run's grant records that fallback, so its evidence belongs to the run.
Diagnostics name only the backend, the engine and the page's kind.

## Artifact kinds

An agent turn places closed semantic objects (`WorkArtifactDataV1`): `answer`,
`document`, `table`, `comparison_matrix`, `findings`, `chart`, `checklist`,
`evidence_collection`, `diagram`, `code` and `browser_resource_preview`. Each
is validated in Rust, and a refused object reaches the model as a closed
notice naming the first field that failed, never its value.

Every finished request publishes exactly one `answer`: `{ kind: "answer",
markdown }`, the reply a careful expert would write in a chat, at most 16 KB
and 400 lines. Its Markdown is a closed subset: level 2 and 3 headings,
paragraphs, bullet and numbered lists nested at most one level, bold, inline
code, fenced code naming a language from the `code` kind's list, and block
quotes. A line scanner in core, not a Markdown parser, refuses raw links and
bare URLs (`AnswerLink`), images (`AnswerImage`), HTML (`AnswerHtml`), level 1
or deeper headings (`AnswerHeading`), tables (`AnswerTable`, since a table is
its own object), unnamed or unclosed fences (`AnswerFence`), deeper lists
(`AnswerNesting`) and empty or oversized text (`AnswerText`); code fences and
code spans are passed over. A knowledge answer that names a URL is refused as
`KnowledgeLink`, as any knowledge object that claims an observed source. The
answer never repeats a table, diagram or code its set holds; it refers to
them by title. `document` is only text the person asked for as a note, brief
or letter. `findings` are cited research facts only: a findings object with an
empty evidence array is refused as `FindingsUncited`, whose notice sends the
prose to the answer.

## Evidence and remaining product seams

The excluded public Luna qualifiers compose this actor with the actual
EngineHost context port, runtime mailbox and `SqliteStore` audit implementation.
The application qualifier additionally uses the [trusted macOS composition](agent-work-composition.md),
actual shell admission, journal and application shutdown. It requires trusted
task completion, durable success, focus isolation and clean native/store/worker
teardown.
Only an explicit synthetic qualification can enable retained provider logs;
production/BYOK remains `store:false`. Local diagnostics contain only closed
states, correlations, counters and timings; no page/provider data or secrets.

The first exact authenticated read-only workflow proves one user-attested disposable-workspace task, same-document progressive
inspection, source-bound extraction, durable terminal publication, and clean
retained-resource reuse. It does not implement or qualify native account
discovery, writes, arbitrary Notion pages, Windows, or concurrent Browse.

Next: trusted product task/plan authoring and a user-facing Work command over
the opt-in Rust desktop admission port. No UI/IPC authority is added, and the
full Tauri window/bootstrap path is not live-qualified. Local/hosted model
transport adapters must share the same session semantics. Native macOS
suspend/resume, broader authenticated/public multi-site qualification,
concurrent Browse interaction, navigation and richer tool adapters remain
open. A suspension
request currently revokes and closes: it is `Cancelled` only if all original
owners drain, otherwise recovery. It does not claim an unimplemented native
suspend operation succeeded or retain resumable execution.

The production extraction result is delivered once in memory after durable
terminal acknowledgement. Explicit durable-profile tasks can additionally
publish an atomic private [artifact](agent-work-artifacts.md); historical reads
never change a prior execution disposition. Content never enters the existing
audit/journal, and unsuccessful closed runs publish no artifact.
