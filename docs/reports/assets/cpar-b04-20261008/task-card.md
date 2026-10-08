# Task card: release-readiness B-04 / P2

Status: IN_PROGRESS. Fixed base: `08812db2373583b8a725b80071efe75b69f9d7ae`.
Branch: `codex/prism-v4-delivery`. This finding is neither CPAR-04 nor all of #32.
B-03 accepted source remains `5e51f5f170a01b813631824b004eb2fcbf190201`.

Scope classification: `L`, because the observed legal retry requires consistent Attempt-level
identity across runtime/router/protocol/store/control and related contracts. This is a local
B-04 slice; it does not change the Phase branch or complete #32. Main agent owns every edit,
design decision, command execution, verification and commit. Scouts only read/search/review.

The initial card did not record S/M/L or a numeric budget before exploration. This is a process
deviation, not retrospective evidence of meeting that rule. The detailed remaining plan is:

| Substep | Working budget | Completion evidence |
|---|---:|---|
| Actual-read RED and protocol boundary | 20 min | retained failed receipts, fixture and source hashes |
| Minimal bootstrap and exact multi-Attempt persistence | 25 min | focused GREEN, actual async writer/billing/read models |
| Regression and clean committed Full | 20 min | source-pinned receipts, all required gate steps |
| Fixed-base independent Standards/Spec and corrections | 20 min | complete two-axis coverage, no outstanding findings |
| Local report/structured receipt/#57 draft and commit | 15 min | scoped commits, preservation check, docs-only validation |

Budgets measure process only; they cannot lower correctness or cut off required verification.
The actual elapsed time and overruns will be recorded from UTC receipts and Git commit times.
Source changes are limited to the driver's existing error seam, two decoder accessors, router
event ordering, existing Store/control identity/read paths and focused test helpers. Documents:
ADR-0103/index, four directly affected behavior contracts and the B-04 report/assets. No frontend,
OpenAPI, production envelope, provider-native implementation or migration-table change is needed.

Scope: preserve already-validated Usage when source bootstrap fails before handoff,
on its exact failed Attempt, with confirmed persistence and idempotent ledger/read
models. Keep source counters, provenance, input accounting and unknowns. Request
failure, protocol-error retry classification and transport-EOF fallback remain exact.
No request replay to collect Usage. No success/stored/continuation root on failure.

Authorized seams: public gateway HTTP/SSE, real synthetic loopback upstream, observed
actual body reads, real bounded queue/async SQLite writer, production billing worker,
Usage lineage read model and ledger readback. Decoder tests supplement this path.
Messages has a one-event bootstrap handoff; test this protection without inventing
an invalid protocol Usage event. The test-only read capture creates no public API.

Execution: first public Responses RED on unchanged product behavior, minimal repair,
Chat RED on base plus final harness, legal Messages protection, required negative
cases and existing regressions, clean committed checkout Full, independent parallel
Standards/Spec against the fixed base, corrections/rechecks, report/local #57 draft.
Use actual HEAD, dirty state and file hashes for precommit executions; never relabel.

Read rules/contracts: current user instructions, actual AGENTS.md, issue-tracker,
CONTEXT, quality gates, applicable development-plan task rules, ADR-0100 and observer,
attempt/persistence/billing ADRs; read-only #32/#9/#57 snapshots in this directory.
Local commit titles begin B-04. Scope may span existing router/store boundaries only
where required for exact failed-attempt Usage. Persisted identity changes require ADR.

Forbidden: real Provider/account inference, production credentials or environment,
deployment, publish, push, GitHub writes/closure; other A/B/S/E/O repairs. Preserve
original AGENTS.md and unrelated untracked material. Overall release stays BLOCKED.
