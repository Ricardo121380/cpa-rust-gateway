# P5-00 phase-level delivery report

| Field | Value |
|---|---|
| Plan | `v1.9` |
| Task | `P5-00` |
| Date | `2026-07-21` |
| Branch | `codex/p5-anthropic` |
| Status | `DONE` |
| Change Request | `CR-EXEC-007` |
| ADR / Contract | [ADR-0033](../adr/ADR-0033-phase-level-delivery-and-default-ref-cache.md) / [BC-DELIVERY-003](../contracts/BC-DELIVERY-003-phase-level-delivery-and-default-ref-cache.md) |

## Scope

P5-00 implements only delivery infrastructure. It adds no Anthropic request/response behavior,
Provider traffic, database change, public API, or production configuration. P5-01 remained blocked
until the one explicit early GitHub Gate accepted this Task.

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

## Cache and remote evidence

Before this Task, the exact quality-tool cache key existed only under P4 branch/tag refs; no
`refs/heads/main` entry existed. The explicit early Gate was therefore a cold seed, not a warm-hit
benchmark.

| Evidence | Result |
|---|---|
| Workflow | [run 29844498976](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29844498976), immutable SHA `be1eeadc9e08399bc3d0532639496d63725b5724` |
| Fast | PASS in `2m50s` |
| Full | PASS in `8m40s`; cold quality-tool install `8m02s`, supplemental supply-chain check `8s` |
| Required | PASS in `3s`; Docs-only correctly skipped |
| Cache restore | MISS for `quality-tools-Linux-rust-1.97.1-6c77927386864b14a70fbd4b3993fbca77817a4e79756de7e1630982c2584144` |
| Default-ref seed | Cache ID `5928795681`, `refs/heads/main`, `152435049` bytes |

The miss was expected and failed safe by installing and version-checking `cargo-deny 0.20.2` and
`cargo-audit 0.22.2`. G5, not P5-00, records whether the final P5 tag restores this default-ref
entry within the warm-install target.

## Review

The change removes repeated automatic branch runs without adding a user-selectable docs bypass.
Manual dispatch still selects code. The plan guard keeps one IN_PROGRESS Task and prevents a local
P5 result from satisfying P6. No real Provider request was sent and no credential was read.

## Closeout

The reviewed implementation was fast-forwarded to `main`; Fast, Full, and Required passed and the
default-ref seed exists. P5-00 is complete and P5-01 may start as the sole `IN_PROGRESS` Task.
