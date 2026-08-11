# BC-MGMT-013 Protected immutable billing catalog management

| Field | Value |
|---|---|
| Contract | `BC-MGMT-013` |
| Task | `P13-05C` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Domain | Management billing catalog mutation and rollback |

## Endpoints

- `GET /admin/billing/catalogs`
- `POST /admin/billing/catalogs`
- `POST /admin/billing/catalogs/{catalog_version_id}/rollback`

All routes use the P10 management listener and Management Key boundary.  Every request selects one
Config Version through `X-Config-Version`.  Mutations additionally require an exact `If-Match`;
browser-originated unsafe requests require the configured same-origin CSRF token.

## Invariants

1. A list is bounded to 256 immutable catalog versions and returns the selected Config Version
   revision as its ETag.  It never contacts a Provider.
2. Import and rollback are draft-only.  They cannot change the active Config Version.
3. One transaction contains the catalog insert, exact revision compare-and-increment and one
   value-free audit event.  Any conflict or storage failure rolls back all three effects.
4. A catalog identity is immutable and management import is create-only.  Any existing identity
   returns `409 management_billing_catalog_conflict`, including an exact repeated request; it does
   not advance revision or append audit.  Lower Store exact-replay idempotence remains an internal
   crash-recovery primitive and is not exposed as ambiguous HTTP create behavior.
5. A write accepts `operator|imported`, 1 to 512 unique Provider/Channel/public-Model entries,
   bounded identities and non-negative integer micro-unit rates no greater than the JSON safe
   integer `9_007_199_254_740_991`.  Effective timestamps use the same bound so generated
   TypeScript consumers cannot silently lose billing precision.  `test` provenance is
   read-compatible for retained fixtures but cannot be imported here.
6. Rollback is a forward fork: it requires a new catalog identity, copies the predecessor rates,
   records `operator` provenance and retains every predecessor row.
7. Catalog changes do not rewrite existing ledger rows and do not start the billing materializer.
8. Responses omit Secret/ciphertext, credentials, endpoint URL/path, headers/cookies/bodies,
   client-key digests and source-event ids/fingerprints.

## Audit and error semantics

Successful imports append `billing_catalog_imported`; successful rollback forks append
`billing_catalog_rolled_back`.  The audit identity comes from the authenticated management
principal and contains only Config Version, resource kind and catalog identity metadata.
Authentication/CSRF failures retain the P10 fail-closed 404 behavior.  Invalid input is a safe
400; missing predecessor is 404; stale revision and immutable catalog conflict are 409.  All
management responses remain `no-store`.

## Response shapes

Catalog list items expose version id, effective/created timestamps, provenance and the bounded
Provider/Channel/model rate entries.  Mutation receipts expose version id, effective time,
provenance, entry count, `imported|rolled_back`, and an optional predecessor id.  They never expose
an authentication or request payload field.

## Implementation and evidence

- `gateway-store::control_plane` and `gateway-store::billing_ledger` shared transaction helpers
- `gateway-control::management_mutation_service` protected catalog service
- `gateway-http-actix::management_resources` protected routes and safe projections
- `docs/openapi/management-v1.json` and generated management client
- `crates/gateway-http-actix/tests/p13_05c_billing_catalog.rs`
- [`P13-05C report`](../reports/p13-05c-billing-catalog-management.md)
