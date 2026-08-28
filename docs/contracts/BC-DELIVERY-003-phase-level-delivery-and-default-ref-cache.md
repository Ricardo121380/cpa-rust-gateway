# BC-DELIVERY-003: Phase-level delivery and default-ref cache

| Field | Value |
|---|---|
| Contract | `BC-DELIVERY-003` |
| Change Request | `CR-EXEC-007` |
| ADR | [ADR-0033](../adr/ADR-0033-phase-level-delivery-and-default-ref-cache.md) |
| First validation Task | `P5-00` |

## Entry points

- `.github/workflows/ci.yml` defines the main/tag automatic trigger boundary.
- `scripts/check-ci-workflow.rb` validates the trigger and pinned-action structure.
- `scripts/check-plan-state.rb` validates Phase-local Task dependency transitions.
- `scripts/test-plan-state-check.sh` covers accepted and rejected status timelines.
- `docs/06-development-plan.md` defines Phase branch, Task review, and Gate ownership.

## Preconditions

1. The Phase starts from the last accepted Phase target and uses one `codex/p<phase>-<short-name>`
   development branch.
2. Every completed normal Task has an independent commit plus targeted local tests and review.
3. At most one Task is `IN_PROGRESS`; no next Phase starts before the current Phase tag Gate passes.
4. `main` contains the reviewed P5-00 delivery infrastructure before it is used as the default-ref
   cache seed.

## Required behavior

| Situation | Required result |
|---|---|
| Ordinary Phase branch push | Does not start automatic CI and cannot be cited as acceptance. |
| Push to `main` | Runs classified CI; P5-00 uses this once as the explicit infrastructure exception and cache seed. |
| `phase-p*-complete` tag | Always classifies as code and runs Fast, supplemental Full, and Required on the tag SHA. |
| Pull request | Runs classified CI without changing the Phase's single-session Task rules. |
| Manual dispatch | Always classifies as code; it cannot force a code change through docs-only. |
| Phase-local dependency | `LOCAL_PASS_PENDING_PHASE_GATE` may unblock the next Task in the same Phase. |
| Cross-Phase dependency | Requires `DONE`; a local-pass state is rejected. |
| Premature `DONE` | Rejected when an explicit dependency is only local-pass. |
| Default-ref cache hit | Exact tool versions are verified and supply-chain checks still run. |
| Cache miss or mismatch | Tools reinstall with `--locked`; the miss is reported and never bypasses Full. |

## Error and rollback semantics

- A malformed trigger, unpinned Action, invalid state, Fast failure, Full failure, or Required
  failure blocks delivery.
- An ordinary Task branch cannot become release evidence merely because it was pushed manually.
- A cache miss is a performance condition, not an authorization to skip checks.
- Rollback restores automatic CI for ordinary branch pushes and Task-level remote Gates without
  changing application schemas, APIs, or provider traffic rules.

## Corresponding checks

- `scripts/check-ci-workflow.rb`
- `scripts/test-plan-state-check.sh`
- `scripts/test-ci-change-classifier.sh`
- `./scripts/check.sh full`
- P5-00 early `main` Gate
- G5 `phase-p5-complete` Gate
