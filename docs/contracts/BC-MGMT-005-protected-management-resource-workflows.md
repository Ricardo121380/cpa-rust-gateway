# BC-MGMT-005 Protected management resource workflows

| Field | Value |
|---|---|
| Contract | `BC-MGMT-005` |
| Task | `P10-04` |
| Status | Accepted |
| Domain | Protected Versioned Upstream management |

## Entry and preconditions

`configure_management_resources` is the P10-04 registration point. It must be mounted only
through P10-02's `configure_management`, with both `ManagementHttpState` and
`ManagementResourceHttpState` configured. A caller therefore needs an admitted actual peer,
exactly one Management Key, an allowed browser policy and CSRF token for unsafe methods before a
handler can read a body or take a resource/workflow lock.

All reads require one valid `X-Config-Version`. EgressPolicy, Upstream, Endpoint, Credential and
binding graph mutations also require one valid `If-Match: rev-N` against a draft Version. JSON is
bounded to 70 KiB, rejects unknown fields, and uses only the exact P10-01 request shapes.

## Resource lifecycle invariants

1. Every graph mutation is an exact draft-revision transaction: resource mutation, bounded
   `management_resource_audit_events` append and Version-revision increment commit together. A
   success returns the new quoted ETag; a stale Version returns the safe 409 conflict and leaves
   the graph/audit unchanged.
2. Credential create/update accepts plaintext only as write-only input and immediately seals it
   under the Version/Credential association. Reads and mutations return `id`, owning Upstream,
   kind, lifecycle state, record revision and `secret_present` only—never plaintext, ciphertext,
   Header, body preview or Debug value.
3. The binding route owns `endpoint_id`; its body owns only `credential_id`, `enabled`, `priority`,
   `weight` and `concurrency`. The handler loads the Endpoint and Credential, derives their common
   Upstream, and rejects unknown fields, absent resources or a cross-Upstream pair before it asks
   the mutation service to insert the binding.
4. Read-only resource operations expose the Version ETag. Invalid identifiers, malformed bounded
   input, missing required Version/revision headers, absent resources, inactive Version writes and
   internal failures return only fixed value-free management error envelopes.

## Bounded workflow invariants

1. `ManagementEndpointWorkflow` receives only typed Endpoint/Credential identifiers. Its default
   implementation has no Provider client, OAuth material, URL/Secret/Cookie input or send path.
2. Endpoint test returns only outcome, status class and Canonical-lifecycle completion. Catalog
   preview returns only added/removed/unchanged counts. Neither response includes model names,
   request/response values or transport diagnostics.
3. Catalog apply first proves that the exact Endpoint is present at the requested draft revision.
   A stale preflight returns 409 without invoking the workflow. A successful workflow is followed
   by one atomic `catalog_discovery_applied` audit/revision transaction; a final concurrent
   revision conflict remains fail-closed and never writes the audit record.
4. OAuth start/status/cancel return only Credential ID, safe state and optional expiry. Cancel
   writes `credential_oauth_cancelled` to the append-only resource audit. No token, URL, device
   code, verifier, Cookie or response payload enters the HTTP result or audit record.

## SPA lifecycle invariants

The page constructs `ManagementApi` only after a user submits non-empty Management Key and CSRF
fields. Both values remain in the module closure and are supplied through generated-client
callbacks. The SPA uses relative same-origin generated operations only, updates its visible
revision from a returned ETag, never stores the values in local/session/indexedDB/Cookie/URL, and
renders only safe responses. A reload constructs a fresh page with no client/session.

## Corresponding tests

`management_mutation_service` and `p10_04_management_resources` use in-memory SQLite, synthetic
secrets and a deterministic injected workflow. They cover revision/audit atomicity, Credential
redaction, protected route admission, BindingInput ownership, safe non-streaming/SSE results,
Catalog preview/apply/stale rejection and OAuth state transitions. `check-management-spa.mjs`
checks generated-client use, memory-only inputs and reproducible static build. The browser fixture
uses static assets and synthetic value-free `/admin/` responses only; no test binds a production
listener or contacts a Provider.
