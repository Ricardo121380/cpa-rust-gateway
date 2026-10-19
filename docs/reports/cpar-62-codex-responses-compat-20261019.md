# #62 Codex Responses compatibility increment

Date: 2026-10-19 (Asia/Shanghai). **Increment PARTIAL; actual Codex and #62 remain BLOCKED.**
This report separates faithful summary-only implementation from fixed Build service qualification
and actual client completion. #61 stays OPEN with cost NOT_ADMITTED and its formal dependency intact.

## Actual starting point and ownership

Product Git root: `/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway`.
The live starting branch was `develop`, local HEAD/upstream and live `origin/develop` were
`c39bc07b1d53f831199f21cc5ae5062c987670b2`. The requested feature branch did not exist and was
created from that verified develop: `feature/62-codex-responses-compat`. Public main remains an
independent sanitized history. No reset, clean, stash, rollback or unrelated ref change occurred.

The starting index was empty; 9 tracked modifications and 81 untracked files were recorded with
content hashes, modes and symlink targets and backed up locally before editing. Existing ignored
output, worktrees, compatibility links and historical receipts were retained. This increment owns
only its code/tests, new ADR/contract/report/evidence, four contract supplements and two appended
index entries. The pre-existing portions of ADR README and traceability are excluded from staging.

Root rules, latest cross-boundary entries, branch/layout/domain/issue policies, local CONTEXT,
relevant ADR/contract history, #58/#61/#62 bodies/comments and native dependencies were read.
#62's sole blocker remains #61 (native ID 5906831609); #59/#60 are closed. No other issue is advanced.

## Implementation and field decisions

See [ADR-0113](../adr/ADR-0113-responses-summary-without-effort.md),
[BC-PROTOCOL-009](../contracts/BC-PROTOCOL-009-codex-responses-controls.md),
[source receipts](assets/cpar-62-codex-responses-compat-20261019/public-source-manifest.json) and
[field decisions](assets/cpar-62-codex-responses-compat-20261019/field-decisions.json).

Summary-only reasoning retains the sole summary selector under `openai.responses.reasoning`
with typed explicit-effort Thinking absent. Responses routing requires Reasoning capability;
Chat/Messages, unknown controls and typed Thinking collisions reject. Generic Responses and
fixed Build encoders preserve absent effort and revalidate the closed shape. No core field,
manifest/lock, schema 35, stored payload v1, AAD, key or TTL is changed. Frozen legacy effort
serialization and encrypted old/new envelope reopen, owner isolation and replay survive key rotation.

OpenAI's standard and the fixed Codex source independently represent summary/effort. Frozen public
Grok client code also constructs absent effort. These justify faithful request representation,
not a guessed effective effort or production service acceptance. The public Build source has no
proven mapping to production profile 1.0.50. Fixed headers, endpoint, model identity and declared
capabilities are unchanged; real Grok usability remains NOT_RUN.

| Field | This increment | Remaining qualification |
| --- | --- | --- |
| summary-only reasoning | Public derived JSON/SSE RED 400 -> controlled GREEN; exact absent effort, native summary/Usage and two tool cycles | Production 1.0.50 service acceptance/effective effort not established; no real Provider call |
| namespace | Individually tested declaration, name overlap, serial/parallel, selector and history refusals; namespaced output fails once without flattening | Full namespace/name identity chain, collision/choice policy and fixed profile declaration/output/history qualification |
| client_metadata | Whole field retained in its named root extension, then precisely rejected before send; flat string map and nested negative tested | Fixed service contract for Codex-owned string map; observed JSON blob is a string, never renamed or silently deleted |
| web_search | Explicit false/true/absent variants independently rejected before send | Cache-only semantics, phases/output IDs, actions/results/citations/history and applicable Usage under the fixed profile |

No namespace child is flattened, hosted search is not replaced with a function, and no field or
capability is removed from actual CLI requests. All derived tests are synthetic field-isolated
requests, distinct from unmodified actual CLI execution. No new cost research was performed.

## Verification and actual clients

The initial public summary-only regression reproduced HTTP400 before implementation; its original
RED is retained. The first focused test build also exposed missing imports in the new test module;
that compiler feedback is retained as harness feedback, not a production semantic RED.

Controlled field tests pass for the independent summary-only JSON/SSE slice and exact refusals.
The existing two-related-tool-cycle test is reused with summary-only reasoning across JSON and
SSE chunks 1/7/31 bytes. It preserves full argument strings (including Chinese and an integer above
JS's exact-number range), call IDs, results/history order, native summary and known 20/7 Usage.
Namespace output refusal retains one receive/Attempt and no executable tool completion or retry.

The CLI runner adds `--require-codex-success`, distinct from the historical expected-400 preparation
mode. It writes per-check failures instead of treating preparation PASS as Codex compatibility.
A positive actual Codex scenario remains unqualified while required fields are blocked; the new
gate does not itself certify missing native lifecycle or encrypted evidence behavior.

Final immutable source, explicit validation overlays, Full, actual CLI opt-in, exact logs/hashes,
Standards/Spec reviews, preservation checks and authorized synchronization receipts are recorded
below after execution. **Pending results here are not PASS claims.**

## Remaining ticket and product boundaries

- Actual Codex tool closure, native failure/cancellation and required namespace/search/metadata
  behavior remain BLOCKED. Default expected-400 preparation cannot close that gap.
- Pi's existing sequential explicit-history scope is retained. Failed-Usage display, roughly
  15-second cancellation release, one-second diagnostic FAIL and complete encrypted success
  evidence remain separate limitations; no new cancellation SLA is introduced.
- #61 success metadata retention/confirmation/owner readback remains BLOCKED and cost NOT_ADMITTED.
  #59 rejected-Usage evidence does not supply the missing success companion.
- #58/#61/#62 remain OPEN; #63–#67 are not advanced. Real Provider, accounts/new budgets,
  global client installation/configuration, deployment, release and tags are outside this run.

## Retained intermediate failures and review correction

The first Full stopped at Clippy's unnecessary raw-string delimiter; a focused Clippy then found
the newly shared test helper exceeded its line bound. Both are preserved; response checking was
extracted without changing assertions. Initial Standards review of `e0c385d2` found one P2
contract breach: direct Canonical summary-only raw JSON could contain duplicate keys that Value
merged before Router admission. Public ingress and both encoders already rejected that input;
no public send bypass was demonstrated. A new public Router projection regression reproduced
`Ok(Canonical)` instead of rejection, then passed after using the existing RawExtensions unique
map deserializer in all three routing modes. No new dependency or boundary change was needed.
Initial Spec review found no new implementation bug/scope creep and retained the ticket blockers.
See [initial review](assets/cpar-62-codex-responses-compat-20261019/review-initial.json).
The corrected source receives a fresh complete Full and independent final review; earlier runs
remain historical and are not aggregated into the final result.
