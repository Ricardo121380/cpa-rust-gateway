# ADR-0130: CPA request controls and model names

| Field | Value |
|---|---|
| Status | Accepted |
| Date | 2026-10-25 |
| Task / Matrix / Contract references | #69 / OPT-02; ADR-0125/0126/0127; BC-PROTOCOL-015/016; BC-ROUTE-002 |

## Context

Start from the verified integration baseline `84fbfbf778b582f746c1cafe31a2ce58962e8f77`.
CPA reference is v8.0.40, `d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683`.
This limited successor changes the former rejection of Messages budgets 0/-1 in ordinary
CanonicalBridge, adds frozen model normalization and mapped service tiers, then adds explicit
model prefixes and thinking suffixes. ADR-0126 effort extraction and precedence remain.

## Decision

Source extraction, candidate model normalization and final protocol fields are separate steps.
Frozen static thinking metadata describes syntax/ranges only; compiled Reasoning capability,
route grants and credential admission remain independently required. Unknown models keep the
existing explicit compatibility mapping; no live catalog or account capability is inferred.
Same-protocol native retains valid JSON controls. Explicit LosslessBridge retains its contract.
Malformed controls and known invalid history fail independently of winning controls.

Request controls P0 precedes model names P1. Prefixes are additional version-scoped names of
an existing public model and route, with exact existing names winning. Valid suffixes reuse
request control normalization; authorization applies to the resolved model before execution.
No new routing policy, payload scripting, provider or frontend is introduced.

## Consequences

Actual-client #62/#64, fixed-service, real Provider, remote CI and release qualification remain
separate. A requested service tier is not a priority-service guarantee. Existing names and
stored canonical payloads remain readable. Detailed matrices and sources belong to the support
and implementation reports accompanying this decision.

## Alternatives considered

A mutable provider catalog would make frozen differential behavior drift. A separate name
router would duplicate permissions and counting. Reimplementing effort would regress ADR-0126.

## Validation and rollback

Public JSON/SSE independent receiver tests, management publish/rollback, count/inference name
and permission regressions, tool continuation and required Full checks bind to actual source,
lock and binaries. Reverting this ticket must preserve earlier integration and evidence.
