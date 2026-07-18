# BC-CORE-002 Canonical request

| Field | Value |
|---|---|
| Contract | `BC-CORE-002` |
| Task | `P1-02` |
| Status | `DONE` |
| Domain | Framework-independent core |

## Entry

An inbound protocol adapter constructs one `CanonicalRequest` after it has accepted and parsed an
external request, but before public-model resolution, routing, credential selection, or provider
execution.

## Preconditions

- `gateway-core` receives only protocol-neutral request semantics, never HTTP headers, HTTP body
  types, Actix values, provider types, credentials, routes, or endpoint selections.
- The requested model is retained as the client supplied model reference; it is not a resolved
  `PublicModelId` or an upstream model selection.
- Unknown JSON fields are admitted only as explicit raw extensions. Their values are valid JSON,
  are not interpreted by the core, and must never be logged through an error diagnostic.

## Event sequence

```text
Accepted external request
  -> inbound protocol parse
  -> CanonicalRequest
  -> later public-model resolution and routing
  -> later provider request encoding
```

## Invariants

- Message order and content-part order are semantic and remain unchanged by construction, clone,
  serialization, and deserialization.
- The core models text plus historical Tool Call and Tool Result request content. An inbound
  adapter explicitly wraps an unsupported future content kind in the canonical
  `{"opaque":{"raw":...}}` envelope; it is not automatically inferred from an unknown external
  tag and does not claim media semantics.
- A Tool declaration retains its name, optional description, JSON Schema, and extensions. The core
  neither validates Tool arguments nor normalizes empty arguments to `{}`.
- Thinking represents only an explicit client effort request. When the optional request-level
  Thinking object is present, it contains one non-empty open-ended effort label; it is not encoded
  in a `-thinking` model suffix and has no provider-specific mapping, budget, usage, or replay
  state.
- `prompt_cache_key` and `prompt_cache_retention` are retained verbatim as request semantics. This
  task does not derive cache identity, affinity, persistence, or provider cache behavior.
- Each request, message, content part, Tool declaration, and Thinking request may carry explicit
  raw extensions. Extensions live in a named namespace rather than flattened JSON keys, so they
  cannot collide with canonical fields.
- Raw JSON extension values and Tool schemas remain raw JSON values. The contract preserves each
  value, not byte-for-byte formatting or key ordering of an entire input document.
- An extension namespace has unique field names. Duplicate fields are rejected at a JSON boundary
  and by the in-memory construction API; a later value must never silently replace an earlier one.

## Error semantics

- Malformed JSON supplied for a raw schema, raw content, or raw extension is rejected at the
  serialization boundary; it is never coerced into a string or silently discarded.
- A supplied Thinking object without an explicit non-empty effort is rejected; absence is expressed
  only by omitting the optional request-level Thinking object.
- Request diagnostics redact client-supplied request values, including messages, Tool text,
  cache fields, thinking labels, and opaque raw JSON.
- A later protocol bridge that cannot losslessly represent a retained raw extension, Tool,
  Thinking, or structured content must reject that candidate. It must not delete the information.
- `CanonicalEvent`, Tool argument streaming, `{}` normalization, HTTP error encoding, and provider
  execution are outside this contract and remain in later tasks.

## Corresponding tests

- A desensitized fixture containing ordered messages, all supported content forms, Tool schema,
  Thinking, cache fields, and nested raw extensions round-trips through JSON and the in-memory
  structure without loss.
- Unit tests verify raw JSON subtrees remain unchanged, structured extension construction and
  enumeration are lossless, duplicate extension keys and invalid raw JSON are rejected, and
  diagnostics redact client-supplied values. They also reject a supplied Thinking object without
  an explicit effort, including `null`, an empty object, or an empty effort label.
