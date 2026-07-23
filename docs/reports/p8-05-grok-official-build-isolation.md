# P8-05 Grok Official / Build state isolation report

| Field | Value |
|---|---|
| Plan version | `v1.38` |
| Task | `P8-05` |
| Date | `2026-07-23` |
| Branch | `codex/p8-official` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P7-G7-001` |
| Scope / budget | `M`; synthetic state handoff/isolation only; no Official request, account, server, route, or persisted-state change |
| References | Matrix `C01`、`C03`、`C31`、`C33`、`F07`、`G24-G27`; [ADR-0058](../adr/ADR-0058-grok-official-runtime-isolation.md); [BC-PROVIDER-016](../contracts/BC-PROVIDER-016-grok-official-runtime-isolation.md) |

## Delivered evidence

`GrokOfficialRuntimeState` now converts only P8-03's sanitized complete Header windows into the
Router's exact binding-wide quota target. It holds one fixed `grok.official` Provider identity,
one explicit Endpoint/Credential pair, and an injected quota registry. Official Headers cannot
address a Build/Web target. Empty/partial state has no effect, reset arithmetic is checked, and the
Router's own controlled-recovery rules remain authoritative.

The existing Official inference adapter has an opt-in constructor that accepts this explicit
state handoff. After transport headers are available it records the one successful observation or
returns the classified pre-start failure. The default adapter remains metadata-only, so no caller
silently acquires runtime state.

The Official API-key path is explicitly stateless for continuity: it has no Build OAuth, billing,
catalog, cache affinity, response ownership, or reasoning replay import. A same-named public model
is still only routeable through an explicit candidate; it does not merge Provider state.

Failure ownership is value-free and narrow: 401 requests only Official credential replacement,
unknown 403 is egress-local/non-mutating, 429 cools only the selected Official binding, and
408/5xx request only Official endpoint cooling. No classification reads a body or has a Build/Web
state action.

No xAI endpoint, credential source, OAuth cache, server process, account, route, proxy/TUN rule,
or production configuration was read or changed. No Official E2E was sent. G7 remains blocked on
Kiro reauthentication; P8 cannot close, merge, release, or claim `DONE` before the CR
prerequisites pass.

## Verification and review

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p8_02_official_responses --test p8_05_official_build_isolation` | PASS; 9 synthetic tests include the opt-in adapter handoff plus exact Header-to-quota mapping, Build catalog/quota/affinity immutability, stateless Official continuity, 401/403/429/503 ownership, 429 reset, empty metadata, invalid time, and redaction. |
| `cargo fmt --all -- --check`, `cargo clippy --locked -p provider-grok --all-targets --all-features -- -D warnings` | PASS. |
| Full local gate | PASS; [`p8-05-local-full-check.md`](p8-05-local-full-check.md) records Shell/CI/plan/format/Clippy/workspace tests/source and crate boundaries/document links/Secret checks/dependency policy/RustSec audit all passing. |
| Focused code review | PASS; verified the opt-in Adapter invokes the safe handoff exactly once after headers, the one-way Provider-to-Router dependency remains sanitized, no Build/Web runtime type is imported, targets are Endpoint/Credential exact, 403/429 ownership is safe, and diagnostics stay redacted. No findings. |

## Rollback and next task

Rollback removes only the P8-05 Official runtime module, synthetic isolation test, ADR, contract,
report, and traceability/index links, restoring the P8-03 metadata-only boundary. It has no
external effect. P8-06 is next only after local Full Gate/review; it owns the synthetic Official
differential, load, and error matrix, not a real Official E2E under the current CR.
