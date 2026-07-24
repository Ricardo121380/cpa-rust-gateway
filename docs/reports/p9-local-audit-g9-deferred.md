# P9 local audit and G9 deferral

| Field | Value |
|---|---|
| Plan version | `v1.42` |
| Scope | `P9-01` through `P9-08` local evidence review; `P9-09` and `G9` deferral |
| Date | `2026-07-23` |
| Branch | `codex/p8-official` |
| Status | `DEFERRED_EXTERNAL_CANARY` under `CR-P9-LOCAL-001` |
| External state | No browser/profile/Cookie/server source was read; no Web, Statsig, gRPC-Web, Official, or Kiro request was sent. |

## Local evidence

P9-01 through P9-08 are all `LOCAL_PASS_PENDING_PHASE_GATE`. Their isolated reports cover SSO
credential lifecycle, browser-egress binding, fixture Chat/SSE grammar, Conversation binding,
Statsig signer admission/cache, source-labelled quota, 403 owner attribution, and default-off Tool
emulation. The final P9-08 change is commit `ef5f14a`.

| Check | Result |
|---|---|
| P9-08 focused test and Clippy | PASS; 3 tests cover default byte stability, explicit `Emulated` metadata/no native Tool capability, unsafe Tool rejection, and duplicate-schema-key rejection. |
| `cargo fmt --all -- --check` | PASS. |
| `./scripts/check.sh docs` | PASS; 263 Markdown files, plan-state (115 tasks, 0 `IN_PROGRESS`), tracked Secret scan, and whitespace checks passed. |
| `./scripts/secret-scan.sh --all` | PASS. |
| `./scripts/check.sh full` | PASS; shell/CI checks, plan guard, tool-cache behavior, format, workspace Clippy/tests, crate boundaries, document links, Secret scan, dependency policy, and RustSec audit passed. |
| Focused review | PASS; P9 remains fixture-only. The default Tool flag changes no prompt bytes, never advertises native Tool support, rejects ambiguous duplicate-key schemas, and introduces no send path. |

## G9 decision and boundary

`P9-09` and `G9` remain deferred, not passed. Their missing evidence is a P9-owned Web test
account plus explicit Canary authorization. That work must separately establish the live request
contract, current browser/SSO state, egress/WAF behavior, Feature Flag safety, protocol-drift
handling, and circuit behavior. Local fixtures do not prove any of those facts.

This audit created no Delivery tag, remote gate, push, merge, release, route, server change,
browser action, Cookie import, or real Web request. It does not use or replace the separate final
external-authentication package: `P7-09/G7` still requires Kiro OAuth and `P8-07/G8` still
requires its own Official API Key, explicit one-probe authorization, and acceptance criteria.
P10 remains blocked on G9.

## Resume criteria

Before reopening P9, register a new explicit authorization for one P9 test account and bounded
Canary profile. Run P9-09 and G9 against that authorization, review the evidence, then perform
the single Phase Delivery Gate. Do not substitute P7 OAuth, an Official API Key, fixture output,
or an unapproved browser session for the P9 proof.
