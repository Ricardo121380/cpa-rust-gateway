# BC-PROVIDER-002 OpenAI-compatible Responses outbound request assembly

| Field | Value |
|---|---|
| Contract | `BC-PROVIDER-002` |
| Task | `P3-01` |
| Status | DONE |
| Domain | OpenAI-compatible Provider request construction |

## Entry and boundary

`OpenAiResponsesRequestBuilder::build` accepts a typed Base URL/inference-path endpoint, one
request-scoped bearer credential, the already selected upstream model, one `CanonicalRequest`, and
the requested `ResponseMode`. It produces one typed outbound target, three standard headers and a
complete JSON byte body. It has no HTTP client, socket, TLS, proxy, timeout, retry, router,
Credential lease, SQLite connection or response/stream decoder.

P3-02 must consume this target through P2-09 EgressPolicy DNS admission and a bounded shared client
pool. P3-03/P3-04 choose Candidate and Credential before calling this builder. P3-06 owns any
attempt and first-semantic-event retry policy.

## Preconditions

- `base_url` is an HTTP(S), host-bearing URL without user-info, query, fragment, percent escape,
  duplicate separator or traversal. Its raw path is checked before URL parsing can normalize dot
  segments.
- `inference_path` starts with `/`, is path-only, and contains no traversal, duplicate separator,
  query, fragment or encoded ambiguity.
- The caller has selected a non-empty upstream model and supplied a non-empty visible-ASCII API key.
- The Canonical request was admitted by the P1 OpenAI Responses decoder or otherwise satisfies the
  same structural invariants.

## Output mapping

| Source | Outbound representation |
|---|---|
| Base URL `https://relay.example/v1` + `/responses` | `https://relay.example/v1/responses` |
| Selected Candidate upstream model | JSON `model`; public requested model is not forwarded |
| `ResponseMode::Streaming` / `NonStreaming` | JSON `stream: true` / `false`; `Accept: text/event-stream` / `application/json` |
| API key | `Authorization: Bearer <request-scoped key>` |
| All requests | `Content-Type: application/json` |
| Canonical messages | Ordered Responses `input` message/function-call/function-call-output items |
| Canonical Tool result output | A JSON string, or an array with a recognized `input_text`, `input_image` or `input_file` type/value shape; other raw JSON is rejected |
| Tools, Thinking, cache fields | `tools`, `reasoning`, `prompt_cache_key`, `prompt_cache_retention` |
| Root `openai.responses.<field>` extension | Original root `<field>` JSON member |

`CanonicalRequest` messages originally decoded from `instructions` are normalized to an equivalent
developer `input` message; P3-01 never synthesizes a new `instructions` field. Assistant text is
encoded as `output_text`; user/developer/system text is encoded as `input_text`.

## Invariants

- Base URL path is retained when appending a leading-slash inference path. Both the raw Base URL
  path and parsed path reject traversal before a URL-normalization step could hide it.
- The typed endpoint, request, API key and raw body redact their values in `Debug` output.
- Root extensions without the `openai.responses.` namespace, collisions with known fields, empty
  extension names and malformed raw JSON fail closed. They are never silently omitted.
- Historical Tool calls require assistant role; Tool results require tool role, `is_error == false`,
  and a Responses-supported output form. An errored Tool result or an unsupported raw output value
  has no proven P3-01 Responses input encoding and is rejected.
- No request uses a public model name after Candidate selection. No plaintext Credential is read
  from Store or retained in a configuration/snapshot type.
- P3-01 performs no DNS resolve, egress connection, redirect, HTTP write, response parsing or
  FirstSemanticEvent transition.

## Error semantics

| Condition | Gateway error |
|---|---|
| Invalid endpoint Base URL/path | `EgressRejected/Egress` |
| Empty or header-unsafe API key | `CredentialUnavailable/Credential` |
| Empty upstream model, unsupported Canonical field, extension collision or invalid raw value | `UpstreamProtocolError/Provider` |
| JSON serialization invariant failure | `InternalError/Internal` |

Error values contain no raw URL, Credential, Header, request body or upstream diagnostic.

## Corresponding tests

- `gateway-upstream::endpoint_url::tests` verifies retaining Base URL paths, rejection of literal
  and encoded Base URL traversal before normalization, ambiguous URL/path shapes and target redaction.
- `provider-openai-compatible::openai_responses::tests` verifies Canonical decode → outbound build
  → decode Round-trip, selected-model rewrite, stream/Header mapping, secret/body/target redaction,
  extension collision/foreign namespace rejection and unsupported Tool-result rejection.
