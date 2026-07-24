# ADR-0072: Protected management resource workflows

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-24` |
| Task | `P10-04` |
| Matrix / Contract | `H01-H02`、`H21`、`J02`、`J08-J09`; [BC-MGMT-005](../contracts/BC-MGMT-005-protected-management-resource-workflows.md) |

## Context

P10-01 froze the management resource contract, P10-02 admitted only a separate Management Key,
actual private/loopback peer and explicit browser CSRF policy, and P10-03 supplied the generated
same-origin client. The remaining P10-04 resource handlers must mutate a draft configuration
without returning a Credential Secret, bypassing the protected Scope, performing a generic fetch,
or quietly treating a Catalog/OAuth workflow as a Provider capability.

## Decision

- `gateway-http-actix::management_resources` mounts only the P10-01 P10-04 routes by calling
  P10-02's `configure_management`. Every request therefore retains its Management Key, actual-peer,
  Origin/CSRF and fixed audit-principal checks; the public data plane is not changed.
- `ManagementMutationService` owns all EgressPolicy, Upstream, Endpoint, Credential and binding
  graph mutations. A graph mutation accepts an exact draft revision, writes its bounded resource
  audit event in the same SQLite transaction, advances the Version revision and returns it as an
  ETag. Credential plaintext is borrowed only for immediate AEAD sealing; every response uses a
  secret-free `CredentialView`.
- A binding body contains only its contract-owned Credential and scheduling fields. The Endpoint
  and common Upstream identities are derived from the route and stored resources, and a
  cross-Upstream binding fails closed. This preserves the frozen `BindingInput` contract and the
  schema's composite ownership invariant.
- Endpoint test, Catalog preview/apply and OAuth start/status/cancel use an injected,
  identifier-only workflow seam. The default workflow has no Provider transport, credential
  material or outbound request. Catalog apply checks the exact Endpoint/revision before invoking
  the seam and, on success, records the graph-affecting audit action atomically with its revision
  advance. OAuth cancellation records its bounded resource audit action.
- The P10-04 SPA holds the Management Key and CSRF token only in its live module closure after an
  explicit form submission. It calls only the generated client, advances the displayed revision
  from ETags, and clears page-local values on refresh. It cannot write browser storage, construct a
  remote base URL, preview a request body or render a Credential Secret.

## Consequences

P10-04 provides usable protected resource controls and safe, bounded workflow controls without
starting a management listener, serving the SPA, publishing a Snapshot, changing a Provider route
or making a real OAuth/Provider request. A production workflow implementation must be explicitly
injected and keep its own admitted Endpoint/Credential handles; it cannot derive an arbitrary URL,
Secret, Cookie or Header from the management body.

The append-only resource audit table and Version revision are now durable schema requirements.
P10-05 through P10-08 must reuse this revision/audit pattern rather than adding an independent
mutation path. P10-09 alone owns static-resource embedding and actual browser-policy/listener
integration.

## Alternatives considered

- Let the browser submit Endpoint or Upstream identity in a binding body: rejected because those
  identities are path/stored-resource owned and would contradict `BindingInput`.
- Reuse a generic JSON CRUD handler or arbitrary request proxy: rejected because it would weaken
  contract validation, audit names and the no-egress boundary.
- Run Provider/OAuth work from a default handler: rejected because mounting management routes must
  not create ambient external side effects.
- Persist management tokens in browser storage: rejected because a refresh should revoke the
  page-local client and P10-02 already supplies the server-side authorization boundary.

## Validation and rollback

`p10_04_management_resources` verifies protected CRUD, exact ETags/revision conflicts,
Credential-secret absence, strict BindingInput rejection plus path-owned identity derivation,
deterministic non-streaming/SSE results, Catalog stale-before-workflow rejection, audit-backed
apply, and OAuth start/status/cancel. The SPA check proves generated-client-only traffic,
in-memory credentials, contract-owned BindingInput and reproducible assets. The browser fixture is
strictly local and synthetic.

Rollback removes the P10-04 resource routes, mutation service, resource-audit migrations, SPA
workspace and local fixture. It leaves P10-01's OpenAPI contract, P10-02 admission helper, public
inference routes, published Snapshot, external Provider state, deployment configuration and later
P10 tasks unchanged.
