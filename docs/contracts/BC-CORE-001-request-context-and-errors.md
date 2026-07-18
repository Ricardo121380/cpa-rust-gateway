# BC-CORE-001 Request context and errors

| Field | Value |
|---|---|
| Contract | `BC-CORE-001` |
| Task | `P1-01` |
| Status | `DONE` |
| Domain | Framework-independent core |

## Entry

Create a `RequestContext` when the gateway accepts one external request. It carries a mandatory,
opaque `RequestId` through ingress, routing, provider execution, and later usage reporting.

## Preconditions

- The caller supplies a non-empty stable identifier representation.
- No protocol adapter, HTTP type, provider-specific field, Secret, or raw upstream body enters
  `gateway-core`.
- Authentication has not necessarily completed when the context is constructed.

## Event sequence

```text
Accepted external request
  -> RequestContext(RequestId)
  -> zero or more independently identified Attempts
  -> completion, failure, or cancellation
```

## Invariants

- `RequestId` remains unchanged for the lifetime of one external request.
- `AttemptId`, provider, endpoint, credential, and authentication selection are not fixed in
  `RequestContext`: a request can make multiple attempts before its first semantic event.
- Gateway identifiers are opaque typed values; a value for one identifier kind cannot be passed
  where another kind is required.
- `GatewayErrorCode` uses exactly the frozen 16-category taxonomy.
- `GatewayErrorCode` and `ErrorScope` are separate. Classification must choose the affected state
  owner from bounded evidence rather than infer it from a broad status-code rule. Scope preserves
  distinct Request, Credential, Account, Model, QuotaWindow, EgressSession, Egress, Provider,
  Stream, and Internal remediation owners.

## Error semantics

- An unknown-account-evidence `403` is `EgressRejected` with `Egress` scope, not
  `CredentialForbidden`.
- `CredentialForbidden` is emitted only when credential-level evidence exists.
- `GatewayError` retains no caller-supplied diagnostic text. Its message is a fixed value derived
  from the stable code, so it cannot contain credentials, tokens, request bodies, or raw upstream
  responses.
- Protocol adapters encode internal errors into OpenAI or Anthropic payloads in later tasks.

## Corresponding tests

- `gateway-core` unit tests validate non-empty opaque IDs and request-context correlation.
- `tests/fixtures/core/gateway-error-codes.snap` is the deterministic snapshot of all frozen error
  code labels.
- `gateway-core` unit tests verify that egress rejection and credential forbidden remain distinct
  classifications and remediation scopes.
