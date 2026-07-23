# P9-08 Grok Web explicit Tool emulation report

| Field | Value |
|---|---|
| Plan version | `v1.42` |
| Task | `P9-08` |
| Date | `2026-07-23` |
| Branch | `codex/p8-official` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P9-LOCAL-001` |
| Scope / budget | `M`; default-off local Tool prompt composer and fixture request builder integration. No Web endpoint, browser/profile, Cookie source, server, proxy/TUN configuration, Tool execution, or external request was used. |
| References | Matrix `C30`、`C31`、`D28`、`F17`; [ADR-0068](../adr/ADR-0068-grok-web-explicit-tool-emulation.md); [BC-PROVIDER-021](../contracts/BC-PROVIDER-021-grok-web-explicit-tool-emulation.md) |

## Delivered behavior

`GrokWebToolEmulation` is disabled by default. It emits neither a Tool prompt addendum nor native Tool capability; the emulation-aware builder produces exactly the existing text-only fixture body for a tool-free request and rejects a Tool-bearing request before fixture-body construction.

An explicitly enabled setting surfaces `Emulated` metadata but still advertises only the native `Streaming` capability. It accepts bounded validated Tool declarations and inserts a visible structured `mode=emulated` convention before client text. It creates no native `tools` body field, Tool executor, model-text Tool parser, or live Web capability claim.

## Verification and review

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p9_08_web_tool_emulation` | PASS; three synthetic default-off/no-injection, enabled metadata/prompt, native-capability exclusion, and unsafe Tool rejection tests passed. |
| `cargo fmt --all -- --check`, `cargo clippy --locked -p provider-grok --test p9_08_web_tool_emulation -- -D warnings` | PASS. |
| `./scripts/check.sh full` | PASS; plan-state, formatting, workspace Clippy/tests, source/crate boundaries, documentation links, Secret scan, dependency policy, and RustSec audit passed locally. |
| Focused review | PASS: default remains byte-stable and Tool-free, enabled work stays explicitly emulated, and no hidden native-tool/transport behavior was introduced. |

## Deferred external proof

Fixtures do not prove a current Grok Web Tool prompt convention, response grammar, tool-execution behavior, WAF, account state, or production feature-flag safety. P9-09/G9 remain deferred to a P9-specific test account and explicit Canary authorization; P8 Official E2E and P7 Kiro OAuth remain in the final external-authentication package.
