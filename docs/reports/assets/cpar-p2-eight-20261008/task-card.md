# Task card: release-readiness eight remaining P2 findings

Status: RESOLVED_LOCAL for all eight findings; document closeout is recorded separately.
Initial plan and budgets were recorded before implementation exploration; later checkpoints follow below.
Task level: L (cross-module security configuration, restart persistence and frontend state).
Fixed base: `3e5fe6f5bea803b09f95726c95e6f80b531e11b1`.
Actual branch: `codex/prism-v4-delivery`; initial HEAD equals the fixed base.
Finding IDs: B-05, A-02, A-03, A-04, A-05, A-06, A-07, A-08.
These are release-readiness IDs, not CPAR implementation issue numbers.

## Scope and working budgets

| Group / step | Working budget | Required result |
|---|---:|---|
| Rules, exact source/spec baseline, preservation and focused exploration | 25 min | fixed baseline and eight-item acceptance table |
| Group 1: B-05 versioned external master keys | 60 min | real isolated serve process restart, public ownership and key-version evidence |
| Group 2: A-02 through A-05 onboarding and recovery | 65 min | actual failure reproductions, narrow fixes and positive/negative checks |
| Group 3: A-06 through A-08 account facts and evidence freshness | 40 min | material-aware actions and current-versus-retained evidence checks |
| EgoLite desktop acceptance and grouped regressions | 35 min | actual UI interactions with controlled local responses |
| Fixed-source clean checkout Full, two independent review axes and corrections | 45 min | complete file coverage and source-pinned validation |
| Batch report, evidence index, local #57 supplement and docs closeout | 20 min | exact per-item status and auditable local commits |

Budgets are planning estimates, not permission to omit required work. Record actual
timestamps, overruns and blockers; never retrofit a pre-exploration claim.
Main agent owns design, all edits, final execution validation and commits.
Scouts are generic, read-only, single-turn, `fork_turns=none`; only independent
exploration and review may be delegated. Foundational documents are read by the main agent.

## Authorization and invariants

The user authorizes local frontend/backend edits needed for these eight findings,
including `web/prism`, synthetic tests, isolated processes, EgoLite acceptance,
independent reviews, reports and local commits. Each frontend crossing includes
the exact cross-boundary log and trailer in the same commit. Backend contracts
remain authoritative; clients are generated through sync-contract only.

Preserve the modified AGENTS.md and all unrelated tracked/untracked content;
stage only task-owned explicit paths. No unrelated cleanup, dependency upgrades,
tool ownership changes or redesign. No real Provider/account inference, production
credentials/key rotation/data/service changes, deployment, publication, push,
GitHub writes/closure or unspecified external-model calls.

Existing A-01 and B-01 through B-04 remain independently RESOLVED_LOCAL.
S-01, E-01 through E-05 and O-01 remain independent. Release stays BLOCKED.
ADR-0103 binary rollback incompatibility remains explicit.

## Acceptance ledger

| Finding | Final local status | Implementation commits | RED / GREEN and directed evidence | Final review |
|---|---|---|---|---|
| B-05 | RESOLVED_LOCAL | `63bfc0e0` | [per-item index](evidence-index.json): b05-process-red-receipt.json, b05-red-confirmation.json; 2 GREEN receipts | Standards PASS; Spec PASS |
| A-02 | RESOLVED_LOCAL | `f553b9d8`, `7c6e68d7` | [per-item index](evidence-index.json): a02-oauth-red-target-receipt.json, a02-import-red-target-receipt.json, a02-api-red-target-receipt.json; 5 GREEN receipts | Standards PASS; Spec PASS |
| A-03 | RESOLVED_LOCAL | `f553b9d8` | [per-item index](evidence-index.json): a03-browser-red-actual-receipt.json; 1 GREEN receipts | Standards PASS; Spec PASS |
| A-04 | RESOLVED_LOCAL | `f553b9d8` | [per-item index](evidence-index.json): a04-selected-red-target-receipt.json, a04-tracked-empty-red-target-receipt.json; 3 GREEN receipts | Standards PASS; Spec PASS |
| A-05 | RESOLVED_LOCAL | `f553b9d8` | [per-item index](evidence-index.json): a05-kiro-red-target-receipt.json, a05-kimi-red-target-receipt.json; 2 GREEN receipts | Standards PASS; Spec PASS |
| A-06 | RESOLVED_LOCAL | `96c0a96b` | [per-item index](evidence-index.json): a06-materials-red-target-receipt.json; 1 GREEN receipts | Standards PASS; Spec PASS |
| A-07 | RESOLVED_LOCAL | `96c0a96b` | [per-item index](evidence-index.json): a07-pending-red-target-receipt.json, a07-error-red-target-receipt.json; 2 GREEN receipts | Standards PASS; Spec PASS |
| A-08 | RESOLVED_LOCAL | `96c0a96b`, `a98bfd5e` | [per-item index](evidence-index.json): a08-stale-red-target-receipt.json, a08-refresh-lineage-red-actual-receipt.json; 3 GREEN receipts | Standards PASS; Spec PASS |

RESOLVED_LOCAL requires implementation, all necessary acceptance and both reviews.
Each command receipt binds actual HEAD/dirty status or source hashes; final Full
runs in a clean isolated committed checkout. Pure documentation closeout uses
the docs gate. No local PASS implies remote CI, real Provider or production PASS.

A-02–A-05 implementation checkpoint: nine actual EgoLite scenarios GREEN (ordinary close/reimport, both empty-draft modes, nonempty review, completed OAuth/import/API receipts plus follow-up failure, Kiro/Kimi close/reopen/read). Initial script input/wait failures are not RED. Directed task/store/device/import regression 51 PASS and type check PASS; final gates pending.

A-06–A-08 implementation checkpoint: five actual EgoLite target REDs and GREENs; current-revision negative control retained. Runtime credential revision and ordinary configuration scope added as non-secret optional response lineage. Frontend 480 tests/type/build/contract checks PASS; HTTP inventory 4 PASS; runtime lineage 6 PASS; strict Clippy PASS. Final clean-checkout/source-bound acceptance and independent review remain pending. A-02 import configuration-read failure was additionally exercised with a verified failing-read counter and saved ID visible.

Final source checkpoint: `a98bfd5e01ea51d3894cce4afcbc0324e9142702`; independent initial review retained its A-08 FAIL, then actual HTTP RED/GREEN, 11 refresh and 13 adapter regressions, and final source clean Full 45/45 with Rust 1488 PASS closed the correction. Final EgoLite16, UI480, process40 and crossgroup410 all PASS. Both final review axes PASS with 43 unchanged files inherited plus two full delta rechecks = 45/45.

Initial preservation ledger recorded `2026-10-08T14:24:58.459300+00:00`; final acceptance/review checkpoint `2026-10-08T15:52:41.462300+00:00`. Elapsed wall time to this checkpoint: 87.72 minutes versus the original 290-minute planning budget; no budget overrun. Each executed command retains its own UTC timestamps, duration and source/dirty identity. Initial implementation checkpoints above are historical, not current unresolved statuses. No prerequisite blocks any of these eight local acceptances; independent release gates remain BLOCKED.
