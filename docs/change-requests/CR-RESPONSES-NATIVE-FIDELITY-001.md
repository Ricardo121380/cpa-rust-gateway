# Responses native output fidelity

Status: implemented and locally verified, not released, 2026-09-26. User authorized this as the first
step of the CPAR stability closeout. No new live Provider allowance is implied.

## Defect and decision

Native Grok Build currently loses output item IDs and folds summaries into reasoning
content. Introduce optional protocol-neutral output-item lifecycle events alongside
the existing semantic deltas. The core checks identity/lifecycle and retains opaque,
redacted extensions; the Responses codec owns the allowlisted wire representation.
Existing producers without item metadata keep their existing representation.

Native Responses metadata preserves ordered message/reasoning/function-call items,
IDs, separate summary/content parts, and streaming/final consistency. The existing
BC-PROVIDER-003/ADR-0044 blank or empty-object tool-argument normalization to `{}`
remains intentional; this is not byte-for-byte whitespace passthrough. Other public
protocols retain their established text/tool projection (native envelope IDs have no
equivalent there), while rejecting unrepresentable reasoning metadata. Ciphertext
without authenticated ownership remains unsupported and must fail closed, including
when it appears only in a final snapshot. This does not activate unused continuity
stores, widen routing capabilities, or add retries.

## Acceptance

- Native JSON/SSE through the runtime response projector and public JSON/SSE, followed
  by exact client history replay; distinct IDs and summary/content parts retained.
- Partial deltas completed once; contradictory final data and unowned ciphertext fail.
- Empty reasoning and interleaved tool calls retain lifecycle and ordered identity.
- Existing canonical producers and generic Agent loopback gate still pass.
- Persisted old payloads remain readable; new item lifecycle events require the new
  binary for replay. Binary rollback retains data, but may require a new conversation.

No management OpenAPI, schema migration, frontend or production mutation is required
to prove the protocol change locally. Real OMP acceptance and production browser
acceptance are separate subsequent steps.

Evidence: [final local verification and remaining boundaries](../reports/cpar-native-output-fidelity-20260926.md).
