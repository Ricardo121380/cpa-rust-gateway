# Grok Build / Pi reasoning admission repair

Date: 2026-09-23. Scope: local backend repair and synthetic regression evidence.

## Incident and reproduction

The OMP Desktop acceptance used candidate `9130d0259bb53a82f8af290c994dc31fbdd8e028`,
Pi 0.85.1, `cpar-grok-build/grok-4.5`, and thinking level `low`.
CPAR recorded `CredentialUnavailable` at 2026-09-23 05:14:29 UTC with zero upstream attempts.
Both `requested_model` and the resolved model were `grok-4.5`.

The installed Pi SDK was exercised with synthetic messages and a fake key. An `onPayload`
hook stopped execution before sending; a rejecting fetch implementation counted zero calls.
For `low`, the SDK produced `reasoning: {effort: "low", summary: "auto"}` and
`include: ["reasoning.encrypted_content"]`. The server did not retain the incident's raw body;
this reproduction uses the same SDK and settings, not a recovered production payload.

Before the repair, this request decoded successfully but the Canonical bridge rejected
`include` as `UnknownRequestExtensions`. Removing only `include` exposed the independent
`ThinkingUnsupported` rejection of `reasoning.summary`. Removing both admitted the request.
The runtime discarded those rejection reasons and returned a credential error.

## Change

- Same-protocol Responses Canonical/CanonicalBridge projections retain the reviewed
  `reasoning.encrypted_content` include selector and `auto`, `concise`, or `detailed`
  reasoning summaries. Values are preserved through both generic Responses and Grok Build
  request builders. Unknown selectors, malformed values, and cross-protocol loss remain rejected.
- When every evaluated, model-matching candidate rejects protocol projection, the runtime
  returns the existing `ClientRequestError` (HTTP 400) instead of `CredentialUnavailable`.
  Value-free rejection enums are available through `protocol_admission` debug logs.
- A compatible candidate, missing exact model, provider-scope ambiguity, or a failure before
  protocol evaluation does not acquire this classification. Genuine credential failures keep
  their existing category. No error enum, database schema, or management API contract changes.

## Verification

- Router protocol-transform tests: 28 passed, including Pi-shaped preservation, invalid-value
  rejection, existing tool replay, capability admission, and cross-protocol checks.
- Grok Build request/response tests (`p6_03_build_responses`): 15 passed. The new regression
  runs the Pi-shaped request through projection, Build encoding, and decoding, asserting that
  the complete Canonical request survives. Existing summary streaming tests also pass.
- Routed executor regression: checks provider-scoped and exact-model protocol rejection,
  missing models, ambiguous provider scope, forbidden credentials, and zero attempted calls.
- The captured installed-SDK payloads were replayed against the repaired library: unmodified
  `low`, `off`, and the three diagnostic field-removal variants all passed local projection.
- Clippy passed with warnings denied for the affected gateway/router/Grok targets and tests.
  Changed Rust files passed formatting checks; the diff and fixture/report links were checked.
- Fixture: [synthetic Pi request](../../../tests/fixtures/openai-responses/request-pi-reasoning.json).

The initial repair verification above made no production change or real model request.
The user subsequently authorized deployment, then expanded it to include the committed
Prism V2 frontend. Combined revision `3a8e5af60738c925dcf44d94baf376227870a16c`
was deployed on 2026-09-24; see the [production receipt and limits](../prism-grok-production-20260924.md).
OMP's previously consumed acceptance budget remains unchanged. No real model request was
made during deployment; offline admission and encoding do not establish upstream support
or successful inference.
