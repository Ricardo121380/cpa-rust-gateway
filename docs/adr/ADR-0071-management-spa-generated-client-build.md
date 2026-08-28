# ADR-0071: Management SPA generated-client build

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-23` |
| Task | `P10-03` |
| Matrix / Contract | `H01-H02`、`H21`、`J02`、`J08-J09`; [BC-MGMT-004](../contracts/BC-MGMT-004-management-spa-generated-client.md) |

## Context

P10-01 freezes the `/admin/` operation names and one-way secret schemas, and P10-02 freezes
their HTTP admission boundary. A handwritten SPA client would duplicate routes and required
revision headers, then silently drift when a later P10 task changes the OpenAPI contract. Serving
or embedding the UI now would also contradict the separate static-asset and inference-hot-path
boundary.

## Decision

- `web/admin-ui` is an independent TypeScript static SPA. It has no runtime framework
  dependency; the only locked development dependency is TypeScript `5.9.3` in `package-lock.json`.
  `npm ci --ignore-scripts` is the only dependency installation path used by local and GitHub
  checks.
- `scripts/generate-management-client.mjs` is the sole generator. It reads only
  `docs/openapi/management-v1.json`, rejects a non-contract-only/non-OpenAPI-3.1 source, and
  produces a tracked `ManagementApi` surface with one wrapper per stable `operationId`. Each
  operation retains only method, `/admin/` path, declared parameter locations/requiredness, and
  JSON/binary/no-body shape. A stale generated artifact fails the build.
- The generated client admits only declared path, query and header inputs; it owns
  `X-Management-Key` and `X-Management-CSRF-Token` through in-memory callbacks rather than an
  arbitrary header map. It uses only relative `/admin/...` paths, same-origin credentials, and
  redirect rejection. It does not read or write browser storage, Cookie APIs or a configured
  remote base URL.
- The P10-03 shell contains no `ManagementApi` instance and sends no request. It supplies only
  static navigation and a clear unavailable-state. P10-04 through P10-08 own actual screens and
  may call the generated wrappers only after the matching protected route is mounted.
- `scripts/build-management-spa.sh` emits a deterministic ignored `web/admin-ui/dist` tree. The
  check rebuilds it twice and compares every asset digest, verifies all operation wrappers and
  confirms the CSP/static shell plus credential/non-request constraints. Fast/full local and
  remote checks install Node/npm and run that check. P10-09 alone may embed the output into a
  gateway binary and measure data-plane isolation.

## Consequences

Later UI work receives a generated, reviewed operation surface without granting any live route or
generic fetch capability. The Management Key cannot be accidentally placed in local storage or
passed through a page-defined arbitrary header field. Contract changes that add an operation,
parameter or request-body media type cause generated-source freshness/type/build checks to fail
before a page can use stale assumptions.

P10-03 does not serve static files, construct `ManagementHttpState`, bind a port, mount an
`/admin/` handler, create a browser key-entry flow, persist a key/token, call OAuth/Provider,
perform CRUD, encrypt backups or alter public inference handling.

## Alternatives considered

- A framework SPA with a network-fetched API schema: rejected because an unpinned runtime schema
  or package graph would weaken reproducibility and could make the management shell dependent on
  a remote resource.
- Handwritten endpoint methods: rejected because contract drift would be discovered only during
  later browser work.
- Browser local/session storage for Management Key or CSRF material: rejected because persistent
  administrator secrets expand exposure beyond the explicit P10-02 admission state.
- Embedding/serving the bundle now: rejected because P10-09 owns static-resource embedding and
  the proof that it does not enter the inference hot path.

## Validation and rollback

`check-management-spa.mjs` verifies 65 OpenAPI-generated wrappers, stale-source rejection,
same-origin/redirect behavior, absent browser persistence, no shell request, CSP and two equal
static-build asset digests. It performs no HTTP request, listener bind, key/token read, SQLite
write, Provider/OAuth request or static-asset serving.

Rollback removes only `web/admin-ui` and its generator/build/check integration. It leaves the
OpenAPI contract, P10-02 admission Scope, SQLite, Config Versions, Snapshots, public inference
routes and external systems unchanged.
