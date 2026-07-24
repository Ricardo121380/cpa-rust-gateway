# P10-04 Protected management resource workflows

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P10-04` |
| Date | `2026-07-24` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` — implementation, browser E2E, independent review and local Full gate passed; P10's sole Delivery Gate remains G10. |
| Scope / Task Card | Protected EgressPolicy/Upstream/Endpoint/Credential/binding controls plus bounded Endpoint/Catalog/OAuth workflows; no routing graph, runtime status, publish/rollback, backup/restore, static embedding, listener or Provider send. |
| Matrix / references | `H01-H02`、`H21`、`J02`、`J08-J09`; [ADR-0072](../adr/ADR-0072-protected-management-resource-workflows.md); [BC-MGMT-005](../contracts/BC-MGMT-005-protected-management-resource-workflows.md); [P10-01](p10-01-management-openapi.md) through [P10-03](p10-03-management-spa-generated-client.md) |

## Delivered behavior

The P10-04 handlers are registered only inside P10-02's protected `/admin` Scope. They strictly
decode P10-01 resource bodies, keep reads Version-scoped, guard draft graph writes with exact
`If-Match` revisions, append non-secret resource audits transactionally and return the advanced
revision as ETag. Credential secret input is zeroized after immediate AEAD sealing; all responses
are metadata-only.

The Endpoint/Credential binding review corrected an OpenAPI mismatch before closeout: the frozen
body owns only the Credential/scheduling fields. The handler derives the Endpoint from the route,
derives its Upstream from storage, verifies that the Credential has the same owner and rejects any
client-supplied duplicate identity field. This preserves the schema's composite foreign keys and
does not allow a browser to choose a different binding owner.

Endpoint tests, Catalog preview/apply and OAuth start/status/cancel use a typed injected seam.
`RejectingManagementEndpointWorkflow` is the default and has no Provider/OAuth send path. A
deterministic test implementation covers safe result projection, stale Catalog apply before
workflow invocation, apply audit/ETag advance and OAuth cancellation audit.

The SPA constructs its generated client only after explicit page-local Management Key/CSRF input,
uses returned ETags to advance the entered revision and never sends a request outside the generated
same-origin client boundary. It does not persist or render credential/CSRF values.

## Browser E2E evidence

`scripts/p10-04-browser-fixture.mjs` served the built static SPA at `127.0.0.1:4179` and supplied
only synthetic, value-free management results. It has no Provider transport, persistence,
credential source, proxy or external egress.

| Browser action | Observed result |
|---|---|
| Enter fixture Management Key/CSRF and connect | `Connected in memory`; no prior client existed. |
| Create Upstream | `201`, then page revision advanced from `rev-0` to `rev-1`. |
| SSE Endpoint test | Safe `200` result: `pass`, `2xx`, `canonical_lifecycle=true`. |
| Catalog preview then apply | Preview returned `3/1/8`; apply returned `200`, ETag `"rev-2"`, and the page revision advanced to `rev-2`. |
| Create Endpoint/Credential binding | Fixture accepted only the five contract-owned body fields, returned `201`, ETag `"rev-3"`, and the page rendered the server-derived Endpoint/Upstream ownership. |
| OAuth start, status, cancel | Safe `202` pending, `200` pending, then `204`; no OAuth value was rendered. |
| Inspect browser storage before/after reload | `localStorage`, `sessionStorage` and Cookies were all empty; reload cleared Key/CSRF fields and returned to `Not connected`. |

WebKit emitted one platform-only CSP note that `frame-ancestors` in a meta element is ignored; no
application JavaScript exception occurred. The fixture and browser were stopped after evidence
collection.

## Verification and review

| Command / review | Result |
|---|---|
| `cargo test --locked -p gateway-control management_mutation_service` | PASS; revision/audit/secret-free service regression. |
| `cargo test --locked -p gateway-http-actix --test p10_04_management_resources` | PASS; 2 protected resource/workflow integrations cover create/read/update/delete, ETag progression, cascade removal and secret absence. |
| `cargo clippy --locked -p gateway-http-actix --test p10_04_management_resources -- -D warnings` | PASS. |
| `cd web/admin-ui && npm run generate && npm run check` | PASS; 65 generated operations and two identical static builds. |
| `cargo fmt --all -- --check`, `git diff --check`, crate-boundaries, tracked Secret scan | PASS. |
| `./scripts/check.sh full` | PASS; workspace, documentation, dependency/supply-chain and Secret gates passed. |
| Focused review | PASS after the BindingInput contract/ownership correction and adding HTTP PATCH/DELETE/cascade regression coverage; reviewed route protection, exact revisions, audit transactions, no plaintext/ciphertext response, zero-send default workflow, safe result projection, browser-memory lifecycle and no P10-05 scope. |

## Rollback and next task

Rollback removes only P10-04 resource/workflow handlers, resource mutation/audit persistence,
workspace controls and fixture evidence. P10-01 contract and P10-02 admission remain intact; no
public inference, Provider, deployment or server configuration rollback is needed. P10-05 stays
`PENDING` and has not been started.
