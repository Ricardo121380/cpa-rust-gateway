# BC-MGMT-002 Versioned management OpenAPI contract

| Field | Value |
|---|---|
| Contract | `BC-MGMT-002` |
| Task | `P10-01` |
| Status | Accepted |
| Domain | Contract-first HTTP management surface |

## Boundary

[`management-v1.json`](../openapi/management-v1.json) is the authoritative management API
contract. It is contract-only: no current crate serves its paths and no consumer may infer a
listener, management port, authentication implementation, CORS policy or production availability
from the document alone.

```text
P2 typed versioned graph + P4 safe runtime projections
  -> P10-01 OpenAPI resource and secrecy contract
  -> P10-02 authenticated/private Actix admin shell
  -> P10-03+ individual CRUD, diagnostic, audit and backup implementations
```

Public client requests remain outside this boundary. `/v1/*` continues to use client-key
authentication and has no administrative operation or response field.

## Contract invariants

1. The root security declaration is the distinct header `X-Management-Key`; it must never accept a
   client inference key as an implied administrator credential.
2. Resource reads and structured graph writes use `X-Config-Version`. Each graph write requires an
   opaque `If-Match`; a stale token returns a safe conflict and cannot partially mutate a graph.
3. Upstream, EgressPolicy, Endpoint, Credential, binding, PublicModel, Alias, Route, Candidate,
   AccessGroup/grant and Client Key are explicit resources. There is no raw YAML overwrite,
   generic configuration blob write, arbitrary upstream-call proxy, URL-to-fetch field, or
   unauthenticated management route.
4. Credential plaintext exists only as a bounded write-only input. It is absent from every response
   schema, audit schema and error schema. Client Key material exists only in the creation response
   with an explicit `display_once` marker; all later reads reveal only a prefix and metadata.
5. Errors expose only a stable bounded code and message. They contain no response Body, Header,
   endpoint URL, Cookie, OAuth token, Secret, Client Key digest or ciphertext.
6. Endpoint test, Catalog discovery, Route Explain, runtime status, quota recovery, request
   attempts, audit, OAuth and backup/restore names are frozen in the contract but explicitly tag
   their owning later P10 Task. Naming them does not authorize their execution.

## Deferred behavior

P10-01 does not implement management authentication, role policy, HTTP routing, CSRF/CORS,
localhost/private-CIDR admission, SPA generation, CRUD handlers, Provider test requests, OAuth
execution, backup encryption or restore. P10-02 through P10-09 each need their own implementation
and verification before the declared operation can become live.

## Corresponding tests

`p10_01_management_openapi_contract` parses the contract and proves required operation coverage,
versioned mutation parameters, security-scheme separation, secret and one-time-key schema rules,
deferred-operation metadata, absence of generic proxy paths, and resolution of every local
reference. It performs no network, filesystem persistence, real credential, HTTP listener or
Provider operation.
