# ADR-0011: OpenAI-compatible Responses request assembly

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-19` |
| Task / Matrix / Contract references | `P3-01`; `C16`, `D10/D11`, `L06`; [BC-PROVIDER-002](../contracts/BC-PROVIDER-002-openai-compatible-responses-request.md) |

## Context

P2 publishes only a secret-free route snapshot. P3 must turn a selected OpenAI-compatible
Responses Candidate into a request without forwarding the public model name, logging an upstream
credential, or allowing an ambiguous Base URL/path join to target a different route. P3-02 owns
the actual HTTP client, timeout, proxy and connection behavior; P3-04 owns Credential leasing.

The frozen Endpoint model intentionally separates `base_url = https://relay.example/v1` from
`inference_path = /responses`. Standard URL joining treats the second value as root-relative and
would silently discard `/v1`, so P3-01 needs an explicit composition rule.

## Decision

- `gateway-upstream::EndpointUrl` composes only HTTP(S) origin-bearing Base URLs with one absolute
  path-only inference path. It retains the Base URL path, rejects user-info, query, fragment,
  percent escapes, traversal, duplicate separators and non-path characters. The raw Base URL path
  is checked before the URL parser can normalize literal or encoded dot segments; its target is
  redacted in `Debug`. P2-09 `EgressPolicy` remains the owner of scheme/Host/CIDR/DNS admission
  for every later dial.
- `provider-openai-compatible` owns a pure `OpenAiResponsesRequestBuilder`. Its explicit inputs
  are a typed endpoint, a request-scoped visible-ASCII bearer credential, the selected upstream
  model, a `CanonicalRequest`, and `ResponseMode`; its output is a typed URL, fixed standard
  headers and JSON body, not an HTTP-client request.
- The builder overwrites only the outbound `model` with the selected upstream model, sets `stream`
  from `ResponseMode`, emits `Accept`, `Content-Type` and `Authorization: Bearer` headers, and
  normalizes Canonical messages into Responses `input` items. Tool history, Tool schema, Thinking,
  cache fields and `openai.responses.*` root extensions are retained when representable. A Tool
  result's `output` is forwarded only as a Responses-supported JSON string or array of input text,
  image or file content; other raw JSON values are rejected rather than sent as invalid payloads.
- Cross-protocol root extensions, reserved-field collisions, invalid endpoint/credential shapes,
  unsupported roles, an errored historical Tool result and an unsupported Tool-output shape fail
  closed with a safe typed
  `GatewayError`; the builder never silently drops or logs them. Credential/header/body/target
  values are redacted from `Debug`.
- `serde_json` and `zeroize` are direct, explicit dependencies of this Provider crate and are
  included in the mechanical crate-boundary allowlist. No Store, SQLite, network, proxy, TLS,
  timeout, retry, scheduler, response decoder or stream code is added.

## Consequences

P3-02 can consume the output without reparsing the target or recreating headers. P3-03 through
P3-06 will choose Candidates, lease Credentials, maintain runtime state, execute requests and
apply first-semantic-event failover around this pure builder. The first implementation intentionally
supports only the standard OpenAI-compatible Responses header set because P2 has no persistent
custom Header policy; it does not invent a configuration field outside the locked plan.

## Alternatives considered

- `Url::join` directly in the Provider was rejected because `/responses` would replace a configured
  `/v1` Base URL path and because the generic safe composition rule will be reusable by later
  Endpoint adapters.
- A string-concatenated URL was rejected because it makes query, fragment, user-info and traversal
  handling error-prone.
- Reusing the public HTTP request body unchanged was rejected because P3 must rewrite the model
  after Candidate selection and must not expose public-model routing semantics upstream.
- Adding a network client or Credential decryption here was rejected as premature P3-02/P3-04 work.

## Validation and rollback

Unit tests cover Base URL/path combinations, literal/encoded Base URL traversal rejection before
normalization, malformed target rejection, URL redaction, Canonical Responses Round-trip with
upstream-model rewrite, streaming/non-streaming headers, Credential and body redaction,
root-extension collision rejection, foreign-extension rejection and unsupported Tool-output/errored
Tool history. Rolling back P3-01 removes only pure request assembly; it opens no connection and
changes no SQLite or published Snapshot state.
