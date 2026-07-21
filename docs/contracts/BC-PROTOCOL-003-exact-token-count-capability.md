# BC-PROTOCOL-003 Exact token-count capability

| Field | Value |
|---|---|
| Contract | `BC-PROTOCOL-003` |
| Task | `P5-02` |
| ADR | [ADR-0035](../adr/ADR-0035-exact-token-count-capability.md) |
| Status | `DONE` |
| Domain | Anthropic `count_tokens` admission and exact Provider capability |

## Boundary

```text
POST /v1/messages/count_tokens
  -> bearer authentication
  -> Anthropic duplicate-safe decode
  -> Snapshot public-model / Alias resolution
  -> CountTokensExecution { CanonicalRequest, optional approved RouteId }
  -> exact Provider/local capability OR explicit unsupported error
  -> {"input_tokens": exact_u64} OR safe Anthropic error envelope
```

The boundary has no estimation algorithm, Provider request builder, Endpoint selection, credential
read, retry loop, database mutation, background task, or real network traffic.

## Invariants

1. A successful result is an `ExactInputTokenCount` returned by a declared
   `ExactTokenCountAdapter` or explicitly proven compatible local implementation. There is no
   public constructor from input text and no estimated capability variant.
2. An absent capability returns `TokenCountUnsupported` with `Model` scope. It does not return
   `0`, a partial value, an omitted `input_tokens` success, or another tokenizer's count.
3. The success envelope is JSON with exactly one public field, `input_tokens`. It contains no
   `output_tokens`, estimate flag, route, model, Provider, credential, error code, or diagnostic.
4. A Snapshot request resolves its public Model/Alias before executor entry. The canonical request
   retains the original model reference; its optional `RouteId` is the exact Snapshot-approved
   routing identity. Generic authenticated mode deliberately supplies no Route ID.
5. Missing/invalid authentication returns the same safe Anthropic error shape and bearer
   challenge semantics as the handler boundary. Malformed, duplicate, streaming, or invalid
   count requests return `ClientRequestError/Request`. Unsupported exactness returns HTTP `422`
   with `invalid_request_error`; an unknown/non-visible Snapshot model returns `RouteNotFound`.
6. `Debug` representations and HTTP errors do not render request content, raw extension values,
   Endpoint, Provider, credential, internal error code, or an estimate.

## Deferred behavior

- P5-03 owns outbound Tool delta semantics.
- P5-04 owns lossless bridge admission.
- P5-05 owns Endpoint-aware aggregation and the same-Upstream protocol separation that will
  consume the resolved Route ID.
- P5-06 owns Thinking, Stop Reason, Usage/cache semantics, and response-model rewrite.

## Corresponding tests

- Pure Anthropic count decode accepts valid input without `max_tokens`, retains explicit raw
  extensions, and rejects `stream: true`.
- Provider capability proves exact success and fails closed when unsupported.
- Actix E2E proves an injected counter returns only `{"input_tokens":17}`, preserves an Alias in
  the canonical request while passing the Snapshot Route ID, and never invokes the Responses
  executor.
- Actix E2E proves the default unsupported executor returns HTTP `422` and a safe Anthropic
  `invalid_request_error` with no `input_tokens` or internal error code.
