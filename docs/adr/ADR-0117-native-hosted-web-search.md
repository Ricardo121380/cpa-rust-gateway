# ADR-0117: Bounded native hosted web search with independent Build activation

| Field | Value |
| --- | --- |
| Status | Accepted |
| Date | 2026-10-21 |
| Task / Matrix / Contract references | #62 C5; #58 T02/T03/T08/T10; ADR-0100/0101/0109/0112/0115/0116; [BC-PROTOCOL-011](../contracts/BC-PROTOCOL-011-native-hosted-web-search.md) |

## Context

The observed Codex declaration is `{"type":"web_search","external_web_access":false}`.
OpenAI defines false as cache-only. The frozen Build client has independent hosted output,
phase events and explicit native history, but does not establish that control's execution
guarantee or a source/release binding to CPAR's fixed profile 1.0.50. Those missing service
qualifications do not prevent accurate local software using the published wire contract.
C1-C3 and C4 are implemented predecessors; their Accepted decisions remain historical records.

## Decision

Use the closed declaration, item, phase, citation and history subset in BC-PROTOCOL-011.
Extend the existing ordered ResponsesToolSet envelope with one hosted declaration, independently
of function/namespace identities. Preserve ordinary flat function representation when no native
declaration occurs. Router checks the complete set for auto/none/required and flat named choice;
hosted calls never become client function calls or require fabricated function results.
ParallelTools remains an independent capability. Serial client function enforcement remains
unchanged; multiple sequential hosted actions are not extra client function calls.

Reuse OutputItemStart/End, the native item envelope, and the existing annotation mechanism.
Reviewed search phase events use a closed protocol extension on an empty TextDelta, like the
existing annotation carrier. No new core event variant or payload-v1 field is introduced.
Explicit completed hosted history occupies one assistant message with one closed OpaqueContent
envelope and is revalidated before exact top-level re-encoding. Arbitrary opaque blocks remain
unsupported. Preserve action and optional-field presence; do not fetch cited resources.
The pure codec reuses the workspace's existing url parser; its exact dependency allowlist and
lock edge are updated without adding a package/version or any network dependency to the codec.

Actual Build composition, builder and decoder have independent search qualification. Normal
composition has none and rejects requests before lease/send. A private gateway cfg(test) field
plus an owned transport and the existing dependency test feature provide only local qualification.
Namespace qualification cannot grant search, search cannot grant namespaces or ParallelTools,
and all-features stock compilation has no qualifying composition call. Decoder permission also
requires a current search declaration. No request/header/config/URL override grants admission.

Hold hosted item completion and coexisting client function completion until terminal
identity/order/presence and standard Usage validate.
Phase completion is an observed search-stage event, not response success. Truncation, cancellation,
failed/incomplete search items and contradictory done/terminal snapshots cannot become success.
Reuse the existing no-retry boundary and source/lease release behavior.

## Consequences

This is a local standard Responses slice, with independent controlled evidence. Fixed Build
activation remains BUILD_ACTIVATION_BLOCKED. Original Codex, #62 and the three-client milestone
retain their other conditions. Unknown search statistics, nonzero provider-specific tool_usage,
and #61 cost_in_usd_ticks gain no admission or implicit price from this decision. Missing fees
remain UNKNOWN; ordinary token ledger rows do not establish the Provider's complete search bill.

There is no SQL migration, new reader, Canonical shape, payload version, AAD, key or TTL change.
Existing payload v1 retains these closed extensions; old execution code rejects new semantics.
An explicit envelope round-trip and the existing encrypted StoredResponse reader/reopen checks
cover representation. Readability is distinct from execution compatibility. Preserve coherent
events/ledger/evidence and all referenced keys during separately authorized recovery.

## Alternatives considered

- Convert hosted search to a function or execute it in CPAR: changes executor and history meaning.
- Drop false, use true/default or preview: removes the observed cache-only control.
- Forward arbitrary JSON: lacks a closed schema and permits unreviewed semantics.
- Add a generic hosted-tool platform or change core/persistence enums: unnecessary for this slice.
- Grant activation from a synthetic fixture or a public client: confuses independent evidence layers.

## Validation and rollback

Retain public-listener RED. Verify production compiler/factory, actual leased Build adapter,
independent numeric loopback receiver, writer, JSON and fragmented SSE, citations, all supported
actions and exact later history. Check coexistence and independent qualification, malformed and
oversized declarations before send, output/phase/terminal conflicts, unknown Usage, cancellation,
no retry and resource release. Regress C4, namespace, flat/Pi, summary and Messages. Bind final
Full to immutable source, manifest/lock and enumerated inherited overlays, with independent
Standards and Spec reviews. No real Provider, account, paid request or remote mutation is required.

Rollback restores C5 rejection without rewriting or deleting retained payloads. Old binaries do
not gain new execution authority. This decision authorizes no push, issue/dependency write,
merge, release, deployment or actual service activation.
