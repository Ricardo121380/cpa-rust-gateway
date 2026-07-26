# BC-PROVIDER-022 Anthropic-compatible Messages outbound request assembly

| Field | Value |
|---|---|
| Contract | `BC-PROVIDER-022` |
| Task | Aggregation control plane, `anthropic/messages` Endpoint type |
| Status | PROPOSED |
| Domain | Anthropic-compatible Provider request construction |

## Entry and boundary

`AnthropicMessagesRequestBuilder::build` accepts a typed Base URL/inference-path endpoint, one
request-scoped `x-api-key` credential, the already selected upstream model, one `CanonicalRequest`,
and the requested `ResponseMode`. It produces one typed outbound target, four fixed headers and a
complete JSON byte body. It has no HTTP client, socket, TLS, proxy, timeout, retry, router,
Credential lease, SQLite connection or response/stream decoder, and it does not implement
`gateway_provider::InferenceAdapter`.

Body construction is delegated whole to `protocol_anthropic::encode_upstream_request`, which already
returns one complete serialized Anthropic Messages JSON body. This crate passes the
Candidate-selected upstream model to that codec as its own argument and forwards the returned text
byte for byte; it never rewrites `CanonicalRequest::requested_model` and never re-serializes.

## Preconditions

- `base_url` is an HTTP(S), host-bearing URL without user-info, query, fragment, percent escape,
  duplicate separator or traversal. Its raw path is checked before URL parsing can normalize dot
  segments.
- `inference_path` starts with `/`, is path-only, and contains no traversal, duplicate separator,
  query, fragment or encoded ambiguity. `ANTHROPIC_MESSAGES_INFERENCE_PATH` records the Anthropic
  convention `/v1/messages`; a configured Endpoint may supply another path.
- The caller has selected a non-empty upstream model and supplied a non-empty visible-ASCII
  `x-api-key` value.
- The Canonical request was admitted by the Anthropic Messages decoder or otherwise satisfies the
  same structural invariants.

## Output mapping

| Source | Outbound representation |
|---|---|
| Base URL `https://relay.example` + `/v1/messages` | `https://relay.example/v1/messages` |
| Selected Candidate upstream model | JSON `model`; public requested model is not forwarded |
| `ResponseMode::Streaming` / `NonStreaming` | JSON `stream: true` / `false`; `Accept: text/event-stream` / `application/json` |
| API key | `x-api-key: <request-scoped key>` |
| All requests | `anthropic-version: 2023-06-01` and `Content-Type: application/json` |
| Canonical messages, Tools, Thinking, cache controls, root extensions | Owned by `protocol_anthropic::encode_upstream_request` |

## Invariants

- The header set is exactly `accept`, `anthropic-version`, `content-type`, `x-api-key`, in that
  deterministic order. `Authorization` is never emitted. Anthropic authenticates an API key with
  `x-api-key`, and the gateway's own Anthropic Messages ingress rejects a request presenting both
  schemes with `ClientUnauthorized`, so a doubled presentation would break exactly the
  Anthropic-compatible relay chain (local CPA, Kiro-RS) this Endpoint type exists to serve.
- `anthropic-version` is a compile-time constant, not Route configuration. Its value determines
  what the paired response decoder must accept.
- Base URL path is retained when appending a leading-slash inference path. Both the raw Base URL
  path and parsed path reject traversal before a URL-normalization step could hide it.
- The typed endpoint, credential, and outbound request redact their values in `Debug` output. The
  outbound `Debug` exposes only header names and body length.
- The emitted body is byte-identical to the codec's returned JSON text: the provider adds no member
  and drops none.
- No request uses a public model name after Candidate selection. No plaintext Credential is read
  from Store or retained in a configuration/snapshot type.
- This boundary performs no DNS resolve, egress connection, redirect, HTTP write, response parsing
  or FirstSemanticEvent transition.

## Codec obligations designed against

`protocol_anthropic::encode_upstream_request(&str, &CanonicalRequest, ResponseMode) ->
Result<String, GatewayError>` must fail closed with `UpstreamProtocolError/Provider` for any
Canonical shape the Anthropic Messages wire format cannot express losslessly, and with
`InternalError/Internal` for a JSON construction invariant failure. In particular it must reject,
rather than invent a default for:

- a request with no `anthropic.messages.max_tokens` extension, because Anthropic Messages requires
  `max_tokens` and the Canonical core has no shared output-limit field;
- a root extension outside the `anthropic.` namespace, or one colliding with a reserved root member;
- a message role, content block, Tool definition or Tool result shape with no proven Anthropic
  block encoding.

The provider propagates these errors unchanged.

## Error semantics

| Condition | Gateway error |
|---|---|
| Invalid endpoint Base URL/path | `EgressRejected/Egress` |
| Admitted URL differs from the request target | `EgressRejected/Egress` |
| Empty or header-unsafe `x-api-key` value | `CredentialUnavailable/Credential` |
| Empty upstream model, unsupported Canonical shape | `UpstreamProtocolError/Provider` |
| JSON serialization or transport header invariant failure | `InternalError/Internal` |

Error values contain no raw URL, Credential, Header, request body or upstream diagnostic.

## Corresponding tests

- `provider-anthropic-compatible::anthropic_messages::tests` verifies exact target, exact
  four-header set and absence of `authorization` for text, Tool and streaming requests; body bytes
  identical to the codec's returned text plus Canonical round-trip equality; credential/target/body
  redaction in every `Debug`; Base URL and inference-path rejection; and fail-closed propagation for
  an empty upstream model, a request with no `max_tokens` extension, and a foreign Provider
  extension namespace.
