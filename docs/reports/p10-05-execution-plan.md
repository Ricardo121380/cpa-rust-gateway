# P10-05 protected routing and Client Key workflow plan

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P10-05` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Date | `2026-07-24` |
| Boundary | Draft configuration only; no Snapshot publication, inference attempt, Provider request, runtime health/quota/403 projection, route explanation, backup or restore. |
| Inputs | [Management OpenAPI](../openapi/management-v1.json), [P10-04 workflow contract](../contracts/BC-MGMT-005-protected-management-resource-workflows.md), [P10-05 contract](../contracts/BC-MGMT-006-protected-routing-client-key-workflows.md), [ADR-0073](../adr/ADR-0073-protected-routing-client-key-workflows.md). |

## Delivery scope

P10-05 will add P10-02-protected handlers and same-origin SPA controls for the existing P10-01
operations that manage `PublicModel`, Alias, Route, Candidate, `AccessGroup`,
`AccessGroupRoute`, and `ClientKey`. Every graph mutation will reuse the draft Version's exact
`If-Match` revision, atomic resource audit append, and returned ETag established in P10-04.

The task will exercise the complete administrative graph using the public model name `minimax-m3`:
create the public model, alias, route, candidate, access group, route grant, and one Client Key.
The E2E fixture will stay local, synthetic and value-free except for the Client Key's immediate
one-time display assertion.

## Client Key boundary

`gateway_auth::client_key::ClientKeyService` remains the only issuer. P10-05 will inject a
management-time issuer explicitly; it will not load a Pepper from an HTTP request, environment
variable, browser, database, or the HTTP layer. Issuance persists only the service-produced Prefix
and HMAC digest, returns the complete Key only in the successful creation response, and never
returns it from get/list/update/delete, Debug, audit, report, fixture source, or browser storage.
The response must be created after the transaction commits; a failed persistence/audit transaction
must expose no Key.

## Explicit exclusions

- `GET /admin/routes/{route_id}/explain`, runtime Health/Quota/403 and request tracing remain
  P10-06. P10-05 route validation is structural draft-graph validation only and has no Provider
  or Router runtime handle.
- Config Version publication/rollback and audit-history pages remain P10-07; backup/restore
  remains P10-08; serving/embed/listener/data-plane performance remain P10-09.
- Candidate and access-group eligibility is stored but is not published, selected, retried or
  inferred. No handler can derive an upstream URL, Header, Secret or outbound request.

## Verification sequence

1. Store/service tests cover exact revision/audit atomicity, parent ownership, FK cascade effects,
   strict bounds, Client Key issue-after-commit, HMAC-only persistence, one-time presentation and
   status/expiry update/revocation.
2. HTTP integration tests cover P10-02 admission, request-body closure, ETag progression and
   conflicts, all in-scope CRUD/grant operations, structural route validation and absence of a
   complete Client Key outside the create result.
3. SPA and local-browser fixture tests create the `minimax-m3` graph, show a Client Key only for
   the immediate issue result, and prove Management Key/CSRF plus the issued Client Key do not
   enter browser storage or survive reload.
4. Review checks scope exclusion, all Secret/Key-redaction paths, transaction ordering and the
   no-runtime/no-egress boundary. The Phase-level Full gate remains the single P10 preflight.
