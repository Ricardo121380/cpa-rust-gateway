# Task card: release-readiness eight remaining P2 findings

Status: IN_PROGRESS. Recorded before implementation exploration.
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

| Finding | Actual impact / observable target | RED | Modification | Directed checks | Browser/process | Review | Commit / status |
|---|---|---|---|---|---|---|---|
| B-05 | serve cannot load retained old keys with a new active version | b05-process-red | shared explicit external ring; legacy v1 | 12 deployment tests; strict Clippy | b05-process-complete: 40 PASS | pending final two-axis review | IMPLEMENTED_PENDING_BATCH_GATES |
| A-02 | follow-up read failure erases a completed saving receipt | pending | pending | pending | actual completed receipt + 503 | pending | NOT_STARTED |
| A-03 | ordinary close leaves an already applied version pending | pending | pending | pending | first import, close, second import | pending | NOT_STARTED |
| A-04 | empty existing draft prevents automatic application | pending | pending | pending | empty and nonempty draft | pending | NOT_STARTED |
| A-05 | unknown device outcome loses recovery handle on unmount | pending | pending | pending | close, return, same session read | pending | NOT_STARTED |
| A-06 | API Key authorization failures suggest OAuth reauthorization | pending | pending | pending | real material classification | pending | NOT_STARTED |
| A-07 | retained evidence is labelled as the new invocation result | pending | pending | pending | true then refetch pending/error | pending | NOT_STARTED |
| A-08 | failed runtime read lets an old expired row override new auth | pending | pending | pending | new revision + retained old rows | pending | NOT_STARTED |

RESOLVED_LOCAL requires implementation, all necessary acceptance and both reviews.
Each command receipt binds actual HEAD/dirty status or source hashes; final Full
runs in a clean isolated committed checkout. Pure documentation closeout uses
the docs gate. No local PASS implies remote CI, real Provider or production PASS.
