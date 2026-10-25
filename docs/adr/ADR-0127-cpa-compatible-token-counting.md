# ADR-0127: CPA-compatible token counting with explicit estimates

| Field | Value |
|---|---|
| Status | Accepted |
| Date | 2026-10-24 |
| Task / Matrix / Contract references | User's to-spec/to-tickets refresh and explicit count_tokens decision; #9 story75/T09; #58; ADR-0035; BC-PROTOCOL-003 |

## Context

The production Messages count route currently uses an unsupported executor. ADR-0035 also
requires exactness and forbids an estimation fallback. CPA v8.0.40, pinned at
`d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683`, uses native token counting for the applicable
first-party Anthropic origin and local estimation for custom/third-party origins. See its
[CountTokens implementation](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/runtime/executor/claude_executor_tokens.go#L19).

The user explicitly chose CPA compatibility, permitting clearly identified local estimates.
This is a product decision for subsequent implementation, not evidence that counting is wired.

## Decision

- Prefer the applicable upstream counting capability. Permit a local estimator for supported
  third-party paths, following the pinned CPA behavior and documenting its supported inputs.
- Distinguish provider-reported, proven exact-local and estimated results internally. An estimate
  must never be constructed as `ExactInputTokenCount` or described as a proven model-exact result.
  Upstream-reported means source provenance; it does not promise a future inference will consume
  the same number of tokens.
- Preserve the standard `input_tokens` success body. Publish counting method and limitations in
  support documentation and non-secret diagnostic observations; do not require a custom client
  extension or put prompts, tool contents, credentials or endpoint secrets in diagnostics.
- Route selection, authorization and model visibility remain in the existing HTTP/Router/runtime
  chain. Count requests must not invoke inference, manufacture completion Usage or write a
  billable inference ledger entry. Applicable counting requests may use their own upstream API.
- Unsupported input forms, unavailable counting methods and malformed requests return explicit
  safe errors; neither a fabricated zero nor an unrelated generation response is a fallback.
- The implementation slice must amend the affected contract and test both native and estimated
  paths through the real public handler and production composition. It must describe estimator
  limitations and measured fixture comparisons rather than invent a universal accuracy promise.

This decision replaces ADR-0035's prohibition on estimates only for the future compatible count
flow. Its exact capability remains exact; ownership, safe errors and admission are preserved.
BC-PROTOCOL-003 still describes the implemented historical exact seam until its implementation
amendment lands. This planning change does not change the current unsupported production result.

## Consequences

Clients can use a compatible count endpoint on supported third-party routes without treating the
returned count as a bill. Count provenance and actual inference Usage are distinct. The new
executor/result representation and production wiring remain to be implemented and verified.
The existing three-client release selection and fixed Build qualification are unchanged.

## Alternatives considered

- Keep exact-only rejection: explicitly declined by the user for this compatibility work.
- Label every result exact or reuse the inference Usage/ledger: rejected because estimation is
  not a measured inference and would mislead diagnostics and billing.
- Add a new mandatory JSON field: unnecessary for CPA-compatible clients; documentation and
  safe diagnostics can communicate the method while retaining the standard response shape.

## Validation and rollback

This turn validates only the decision, document links and consistency with the user's answer.
No product code, behavior test, Provider request or production setting changes here. The future
ticket covers official counting, estimation, tool/history inputs, visibility, malformed and
unsupported input, independent receive counts and absence of synthetic inference Usage/ledger.
Reverting an eventual implementation must preserve existing configuration and historical results;
changing this accepted product choice requires a successor decision.
