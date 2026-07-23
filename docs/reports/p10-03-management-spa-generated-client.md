# P10-03 Management SPA and generated client

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P10-03` |
| Date | `2026-07-23` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` — implementation, focused review and the required local Full gate passed; P10's one Delivery Gate remains at G10. |
| Scope / Task Card | Independent static TypeScript SPA, generated OpenAPI client and reproducible local/CI build only; no management listener, CRUD page, Management Key UI/persistence, OAuth, Provider request, backup operation or static-asset embedding. |
| Matrix / references | `H01-H02`, `H21`, `J02`, `J08-J09`; [ADR-0071](../adr/ADR-0071-management-spa-generated-client-build.md); [BC-MGMT-004](../contracts/BC-MGMT-004-management-spa-generated-client.md); [P10-01 contract](../openapi/management-v1.json); [P10-02 admission](p10-02-management-http-admission.md) |

## Delivered behavior

`web/admin-ui` contains a no-runtime-dependency TypeScript management shell, locked TypeScript
compiler and static local HTML/CSS. Its initial render is an explicit unavailable-state/navigation
shell; it never constructs an API client or sends an HTTP request.

`generate-management-client.mjs` converts the frozen OpenAPI contract into a tracked client with
65 named operation wrappers. The client admits only frozen parameters and body encodings, owns the
Management Key/CSRF headers through callbacks, uses relative same-origin `/admin/` targets and
rejects redirects. No browser store, Cookie API, remote base URL or arbitrary request-header escape
hatch is present.

The deterministic build requires `npm ci --ignore-scripts`, refuses stale generated source,
compiles to an ignored `dist` tree and copies only local static assets. `check-management-spa.mjs`
rebuilds twice, compares every SHA-256 asset digest and checks the generated client/shell safety
properties. Fast/full checks and both code CI jobs now install Node/npm and run this build check.

## Targeted verification and review

| Command / review | Result |
|---|---|
| `node --check scripts/generate-management-client.mjs && node --check scripts/check-management-spa.mjs && bash -n scripts/build-management-spa.sh` | PASS |
| `npm ci --ignore-scripts --no-audit --no-fund --prefix web/admin-ui` | PASS; one lockfile-pinned TypeScript compiler only. |
| `node scripts/generate-management-client.mjs && ./scripts/build-management-spa.sh && node scripts/check-management-spa.mjs` | PASS; all 65 wrappers, two identical static builds, fake-fetch same-origin/CSRF/JSON/binary behavior, and pre-fetch invalid-input rejection. |
| `./scripts/check.sh full` | PASS; plan/CI guards, locked npm SPA check, workspace tests, source/crate policy, docs, Secret scan, dependency policy and RustSec audit passed. |

Focused review must verify operation-map/key alignment, required/optional JSON and binary contract
bodies, path/header input rejection, same-origin/CSRF/redirect constraints, no client construction
in the shell, no browser key persistence, deterministic assets and P10-09-only embedding.

## Rollback and next task

Rollback removes this independent static build boundary, leaving P10-01/02, the management OpenAPI
contract, Actix admission Scope, SQLite, inference routes and external systems intact. P10-04 is
next only after P10-03 is locally accepted; it may add Upstream/Endpoint/Credential pages and
handlers through the pre-existing guarded Scope.
