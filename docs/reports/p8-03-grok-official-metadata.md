# P8-03 Grok Official rate-limit and billing metadata report

| Field | Value |
|---|---|
| Plan version | `v1.38` |
| Task | `P8-03` |
| Date | `2026-07-23` |
| Branch | `codex/p8-official` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P7-G7-001` |
| Scope / budget | `S`; fixed Header/usage metadata only; no Official request, persisted state, server, route, or account change |
| References | Matrix `C01`、`C31`、`C33`、`E11`、`G16`、`G26`; [ADR-0056](../adr/ADR-0056-grok-official-rate-limit-billing-metadata.md); [BC-PROVIDER-014](../contracts/BC-PROVIDER-014-grok-official-rate-limit-billing-metadata.md) |

## Delivered evidence

The Official transport now safely projects only fixed `x-ratelimit` request/token triplets and
delta-seconds `Retry-After`. The shared response boundary exposes all values for one requested
Header, so duplicate known headers are rejected instead of being accidentally normalized. Parsed
metadata contains typed counters/durations only; Debug exposes field presence/counts but no raw
Header values.

Provider-reported Canonical token counters can now be projected as Official billing metadata.
This is intentionally not a billing price/plan/account model. Rate limits and token usage cannot
modify runtime quota, credential, account, health, retry, scheduler, persistence, or any Build/Web
state in P8-03. Those transitions wait for P8-05's exact source-isolated state boundary.

No xAI endpoint, credential source, OAuth cache, server process, route, proxy/TUN rule, or
production configuration was read or changed. No Official E2E was sent. G7 remains blocked on
Kiro account reauthentication; P8 cannot close or release before the CR prerequisites pass.

## Verification and review

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p8_03_official_metadata` | PASS; 3 synthetic tests cover fixed triplets/retry/redaction, ambiguous/unsafe failure, and non-financial usage metadata. |
| `cargo fmt --all -- --check`, `cargo clippy --locked -p provider-grok --all-targets --all-features -- -D warnings` | PASS. |
| `cargo test --locked -p provider-grok` | PASS; 59 active tests passed and 2 pre-existing explicitly authorized P6 live harness tests remained ignored. |
| [`./scripts/check.sh full`](p8-03-local-full-check.md) | PASS; workspace format/Clippy/tests, source and crate-boundary policies, document links, Secret checks, dependency policy, and RustSec audit all passed locally. |
| Focused code review | PASS: fixed allow-list, duplicate visibility, raw-value redaction, absence of state/status mutation, and no Build/Web sharing are retained. |

## Rollback and next task

Rollback removes only the P8-03 metadata parser, response duplicate-header iterator, tests, ADR,
contract, report, and traceability/index links. It has no external effect. P8-04 is the next local
task after P8-03 review: it will add explicit Official Tool, Reasoning, and Search capability
declaration/conversion without making a real Official request.
