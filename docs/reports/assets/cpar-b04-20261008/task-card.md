# Task card: release-readiness B-04 / P2

Status: RESOLVED_LOCAL. Final source: `062f8281279b9bc6a0dc36b2859120e26e5f85dd`. Fixed base: `08812db2373583b8a725b80071efe75b69f9d7ae`.
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


## Recorded completion evidence

- Actual implementation commit: `42fae997ef7c82f7e611e97f22da8b5a79634233`.
- Standards traceability correction: `640f4a188336326b58c85266182d8faabad5d725`.
- Spec historical partial-lineage correction/final source: `062f8281279b9bc6a0dc36b2859120e26e5f85dd`.
- Directed 949/0/8 and Store correction 9/0/0 retain their original dirty HEAD/file hashes.
- Full final source: 45/45, exit 0, 1487 passed / 0 failed / 12 ignored; clean checkout before/after.
- Standards PASS and Spec PASS, all 26 source-diff files via complete scoped reviews and final blob proof.
- The initial L/numeric-budget omission remains a process deviation. Five interrupted review rounds are not PASS.
- Actual retained receipts, manual-Store versus real async-path boundary, and public protocol results are in verification.json.

Recorded timestamps are UTC. Earliest timestamped receipt starts 2026-10-08T10:09:51.779624Z;
Full ends 2026-10-08T11:27:20.214834Z, a recorded span of 77.474 minutes through Full.
This is elapsed recorded span, not a measurement of hands-on work; initial exploration and final
artifact closeout fall outside that interval. Source commit times: implementation 18:47:42+08:00,
Standards correction 19:05:06+08:00, final source 19:13:11+08:00. Review corrections were necessary;
no budget limit was used to skip required work. Final closeout elapsed span is recorded separately.

Artifact closeout checkpoint UTC 2026-10-08T11:44:33.560389+00:00: recorded elapsed span from the earliest receipt
is 94.696 min. This is a checkpoint, not final wall time or a retroactive initial budget record.
