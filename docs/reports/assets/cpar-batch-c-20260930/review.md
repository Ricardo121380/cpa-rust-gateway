# CPAR C two-axis review

Fixed comparison: `git diff 45a315a6809eff5ba75ad47c53fe1825a88f7da9...bc9b2b5ac62c27acc62a4f3bf53b1806e85522ea`.

Commit list: `bc9b2b5 fix: preserve batch C provider semantics and record blocked acceptance`.
Two independent read-only agents; line references below identify this immutable reviewed commit.
The main agent owns changes and execution. The reviews did not execute tests or call Providers.

## Standards

**Coverage: complete for the declared changed-source scope.** No documented hard violation found.
Standards checked: AGENTS.md ownership/trace protocol, docs/quality-gates.md:18–21,36–45,
and ADR-0100. No changed path triggers the frontend crossing requirement. Formatting,
Clippy and dependency rules were excluded because tooling checks them.

**Baseline judgment: Duplicated Code, low severity.**
crates/provider-grok/src/build_responses.rs:1757–1776,2037–2046 and
crates/provider-grok/src/official_responses.rs:908–928,1195–1204 duplicate argument buffering,
first-delta suppression and completion fallback. Both contain the branch
`else if could_be_empty_tool_arguments(arguments) { return Ok(()); } else { arguments.clone() }`.
The shared predicate exists; the surrounding emitted/buffer transition remains duplicated.
This is a maintainability observation, not a documented prohibition or demonstrated defect.

**Evidence limitation:** factory tests assert HTTP 200 plus body substrings
(batch_c/http.rs:357–375, batch_c/grok.rs:221–237, batch_c.rs:418–429).
These alone do not establish successful SSE terminals or complete public tool arguments.
The report's “298 次成功请求” is stronger than those inspected assertions. This is an
evidence-strength observation, not a hard Standards violation.

Coverage: every changed production hunk in runtime.rs, catalog_refresh.rs, Chat codec,
Grok Build/Console/Official/strict JSON/Web, and Kiro conversation/event/inference/profile;
all changed test structures; both ADR/BC amendments; acceptance script; report boundaries
and receipt metadata/counts. JSON receipts agree with 40/16/6 requests and old runtime SHA.
Not read: unchanged test bodies, full per-row inventory/pin details, external references,
credentials or unrelated files.

Main-agent disposition: retain separate native decoder ownership and their existing limits;
the low-severity heuristic does not justify broad state-machine extraction in this repair.
Strengthen all 298 successful public responses with complete JSON/SSE decoding, Canonical
lifecycle validation, exactly one success terminal, exact tool ID/name/JSON-object arguments
and text answers. Source lives in `apps/gateway/src/runtime/batch_c/public_response.rs`.

## Spec

**Coverage: PARTIAL; completion verdict: FAIL / C BLOCKED.**

New **P2**: Official/Console SSE logprobs can still be silently discarded.
spec.md:218 requires known semantic losses to be fixed; :219 requires response
incompatibility to fail without a success terminal. validate_part_semantics at
official_responses.rs:1302 checks content-part snapshots, but handle_text_delta:799 and
handle_text_done:827 ignore event-level logprobs. Trigger: a correlated output_text.delta
or .done carries nonempty logprobs; final plain snapshots omit it; handle_response_completed
emits ResponseEnd. Console uses this decoder (console_responses.rs:515). This was a static
inference in the scout review, not an executed reproduction.

Documented incomplete requirements remain: Kiro Messages hard cap, Web native multi-turn
continuity and native metadata support (report:98,104,108; spec:144–146,325); per-channel
declared extensions (report:129–137); and new-build real key loops (spec:342). Old deployed
build evidence cannot accept this HEAD. No scope expansion identified in reviewed changes.

Coverage: specification personally read; production diff and directly related handlers;
report and ticket criteria; sampled new tests. Fixture code and four live JSON attachments
were not exhaustively inspected. No execution, network or credential access.

Main-agent disposition: reproduce P2 with a failing test, then validate text-event metadata
before delta/done processing. Both Official and Console reject the delta/done cases;
the four concentrated provider tests pass after the fix. Known scope blockers remain
open; failure-safe rejection is not support for metadata or completion of a ticket.

Standards: 0 hard violations, 1 low-severity smell and 1 evidence limitation (addressed).
Spec: 1 new P2 (reproduced and fixed), plus incomplete software/extension/live requirements;
completion remains BLOCKED and coverage PARTIAL. Worst within Standards was evidence strength;
worst within Spec is the unfulfilled frozen completion contract.
