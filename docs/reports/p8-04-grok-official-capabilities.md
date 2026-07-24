# P8-04 Grok Official Tool, Reasoning, and Search capability report

| Field | Value |
|---|---|
| Plan version | `v1.38` |
| Task | `P8-04` |
| Date | `2026-07-23` |
| Branch | `codex/p8-official` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P7-DEFER-002` |
| Scope / budget | `M`; local codec/capability work only; no Official request, state, server, route, or account change |
| References | Matrix `B11`、`B12`、`B16`、`C01`、`C03`、`C18`、`C31`、`F02`; [ADR-0057](../adr/ADR-0057-grok-official-tool-reasoning-capability-boundary.md); [BC-PROVIDER-015](../contracts/BC-PROVIDER-015-grok-official-tool-reasoning-capability.md) |

## Delivered evidence

The isolated Official Responses codec now converts Canonical Function Tool definitions, assistant
Tool history, string Tool Results, and explicit `low`/`medium`/`high` Reasoning into the Official
Responses request subset. Non-streaming and SSE decoders convert completed `function_call` and
exported `reasoning` items into a single valid Canonical lifecycle, including bounded Tool argument
delta/final agreement and reasoning-token usage when reported.

Its fixed capability declaration contains Tools, ParallelTools, JSON Schema, Reasoning, and
Streaming. It deliberately does **not** claim Search: current Canonical/ingress contracts reject
native Search tool shapes and have no Search output semantic. The safe status is
`UnavailablePendingCanonicalContract`; Search request/output continues to fail closed rather than
being relabelled or dropped.

No xAI endpoint, credential source, OAuth cache, server process, account, route, proxy/TUN rule,
or production configuration was read or changed. No Official E2E was sent. G7 remains blocked on
Kiro reauthentication, but `CR-P7-DEFER-002` makes P8 independent; P8-07 owns the remaining
separate Official API-key E2E authorization.

## Verification and review

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p8_04_official_capabilities --test p8_02_official_responses` | PASS; 11 tests cover capability truthfulness, request/history conversion, Search refusal, non-streaming/SSE semantic equivalence across byte splits, Tool completion safety, and existing text boundary regressions. |
| `cargo fmt --all -- --check`, `cargo clippy --locked -p provider-grok --all-targets --all-features -- -D warnings` | PASS. |
| Full local gate | PASS; [`p8-04-local-full-check.md`](p8-04-local-full-check.md) records Shell/CI/plan/format/Clippy/workspace tests/source and crate boundaries/document links/Secret checks/dependency policy/RustSec audit all passing (49 s total; 34 s Rust tests). |
| Focused code review | PASS; independently checked no Build/Web profile or runtime-state import, no Search overclaim, stable Tool item/call/name correlation, strict delta/final JSON-object identity, and redacted Debug representations. No findings. |

## Rollback and next task

Rollback removes only the P8-04 Official capability module, Tool/Reasoning codec extension,
regressions, ADR, contract, report, and traceability/index links. It has no external effect.
P8-05 is next only after local Full Gate/review; it owns Official/Build state, affinity, quota, and
failure isolation, not any new Official E2E.
