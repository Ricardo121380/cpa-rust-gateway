# P5-00 phase-level delivery report

| Field | Value |
|---|---|
| Plan | `v1.9` |
| Task | `P5-00` |
| Date | `2026-07-21` |
| Branch | `codex/p5-anthropic` |
| Status | `LOCAL_PASS_PENDING_CI` |
| Change Request | `CR-EXEC-007` |
| ADR / Contract | [ADR-0033](../adr/ADR-0033-phase-level-delivery-and-default-ref-cache.md) / [BC-DELIVERY-003](../contracts/BC-DELIVERY-003-phase-level-delivery-and-default-ref-cache.md) |

## Scope

P5-00 implements only delivery infrastructure. It adds no Anthropic request/response behavior,
Provider traffic, database change, public API, or production configuration. P5-01 remains blocked
until the one explicit early GitHub Gate accepts this Task.

## Delivered controls

- Automatic push CI is limited to `main` and `phase-p*-complete` tags. Pull requests and manual
  dispatch remain fail-closed; tag/manual runs classify as code.
- Ordinary Phase branch pushes no longer start per-Task GitHub runs. The annotated Phase tag is the
  one normal formal Fast plus Full delivery event.
- `LOCAL_PASS_PENDING_PHASE_GATE` is recognized as a non-DONE state. It may satisfy an explicit
  dependency only inside the same Phase; cross-Phase reuse and premature DONE are rejected.
- P5-00's reviewed commit will fast-forward `main` as the explicit infrastructure exception. That
  run creates the default-ref quality-tool cache used by later Phase tags.
- Cache hit/miss still has no authority over quality: exact versions, `cargo deny`, `cargo audit`,
  and Required all remain mandatory.

## Local evidence

| Command | Result |
|---|---|
| `./scripts/check-ci-workflow.rb` | PASS; trigger is restricted to main and Phase tags, required jobs/actions remain present and pinned. |
| `./scripts/test-ci-change-classifier.sh` | PASS; manual dispatch and tags remain code, docs/code path classification remains fail-closed. |
| `./scripts/check-plan-state.rb` | PASS; 114 Tasks, exactly one IN_PROGRESS before local closeout. |
| `./scripts/test-plan-state-check.sh` | PASS; same-Phase local-pass dependency accepted, cross-Phase and premature-DONE timelines rejected. |
| `./scripts/check-shell-syntax.sh` | PASS; 13 scripts. |
| `./scripts/check.sh full` | PASS; Workspace Fast, pinned dependency policy, and RustSec audit completed once. |
| `git diff --check` | PASS. |

## Cache and remote evidence boundary

Before this Task, the exact quality-tool cache key existed only under P4 branch/tag refs; no
`refs/heads/main` entry existed. The P5-00 early main Gate must therefore be treated as a cold seed,
not as a warm-hit benchmark. After it passes, the closeout report will record the immutable run and
the resulting default-ref cache entry. G5, not P5-00, records the final P5 tag restore result.

## Review

The change removes repeated automatic branch runs without adding a user-selectable docs bypass.
Manual dispatch still selects code. The plan guard keeps one IN_PROGRESS Task and prevents a local
P5 result from satisfying P6. No real Provider request was sent and no credential was read.

## Pending closeout

Fast-forward this reviewed implementation to `main`, wait for its Fast + Full + Required Gate, and
confirm a default-ref cache entry. Only then may P5-00 become DONE and P5-01 start.
