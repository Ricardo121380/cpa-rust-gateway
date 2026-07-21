# P5-08 Anthropic adversarial stream properties report

| Field | Value |
|---|---|
| Plan version | `v1.9` |
| Task | `P5-08` |
| Date | `2026-07-22` |
| Branch | `codex/p5-anthropic` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Scope / budget | `M`; deterministic adversarial test evidence only; no production protocol or Provider behavior change |
| Execution channel | Current default model, `medium`; fallback: Luna is unavailable in this execution surface. No subagent was used because test assertions span protocol terminality and shared cancellation semantics. |
| References | Matrix `B09`, `B12-B16`, `B25`, `B27-B28`; [ADR-0041](../adr/ADR-0041-deterministic-anthropic-adversarial-evidence.md); [BC-PROTOCOL-006](../contracts/BC-PROTOCOL-006-anthropic-adversarial-stream-safety.md) |

## Delivered evidence

P5-08 adds a fixed synthetic Anthropic request corpus and two fixed-seed, 256-case protocol
properties. The corpus classifies unknown-field retention, opaque unknown content, duplicate names,
truncated JSON, invalid user Tool use, and incompatible cache control. The properties verify exact
raw extension retention at root/message/content boundaries, then exercise malformed/truncated Tool
event schedules under `catch_unwind` and require that no schedule can produce an Anthropic
`message_delta`, `message_stop`, or completed response.

The new 128-case cancellation property runs the bounded Canonical stream before and after first
semantic delivery, including repeated cancellation. It checks the shared cancellation token, FSE
commit status, transparent-retry gate, post-cancel producer rejection, and consumer terminal shape.

The only dependency change is a `gateway-stream` development edge to the existing workspace-locked
`proptest` version. `Cargo.lock` records that direct test edge; no resolved package version, runtime
dependency, endpoint, credential, database, or product API changes.

## Verification

| Command / review | Result |
|---|---|
| `cargo test --locked -p protocol-anthropic --test p5_08_adversarial_properties` | PASS; fixed corpus plus four fixed-seed/truncation properties passed. |
| `cargo test --locked -p gateway-stream --test p5_08_cancellation_properties` | PASS; 128 fixed-seed before/after-FSE cancellation schedules passed. |
| `cargo test --locked -p protocol-anthropic -p gateway-stream` | PASS; 40 unit/integration/property tests across the changed crates, including existing P5-03 Tool properties. |
| `cargo clippy --locked -p protocol-anthropic -p gateway-stream --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` and `git diff --check` | PASS |
| `./scripts/check.sh full` | PASS; dependency policy, RustSec audit, source policy, complete workspace tests, docs links, and tracked Secret scan passed. |
| Staged Secret scan and code review | PASS; corpus values are synthetic and no test reports raw generated or client input. |

## Review conclusion

The review distinguishes retention from support: unknown fields are asserted to survive as raw
extension values, not reinterpreted as supported Canonical semantics. The malformed Tool property
starts from a deliberately incomplete Tool prefix and permits only rejection or a terminal safe
error; it refuses every successful Anthropic completion shape. This prevents a regression from
normalizing non-empty partial JSON into `{}`.

The cancellation property intentionally exercises both monotonic terminal boundaries. Before FSE,
cancellation leaves FSE uncommitted but closes retry; after explicit downstream delivery, FSE is
already committed and cancellation cannot reopen retry. In both paths a later sender operation is
`Cancelled/Request` and the consumer cannot return a normal response completion.

## Rollback and phase closeout

Reverting P5-08 removes only the corpus, deterministic test targets, their existing locked
development dependency edge, and associated evidence. No migration, service restart, credential
rotation, or external cleanup is needed. All P5 Tasks are now locally accepted; G5 performs the
single Phase-local closeout review and the one remote Fast + Full Delivery Gate before P6 may start.
