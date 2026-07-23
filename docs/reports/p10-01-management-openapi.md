# P10-01 Versioned management OpenAPI contract

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P10-01` |
| Date | `2026-07-23` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` — P10-01 local implementation and review evidence are complete; P10 remains on its one Phase branch and must not trigger a Delivery Gate before G10. |
| Scope / Task Card | Contract-only OpenAPI resource surface; no management HTTP listener, authentication, CORS/CSRF policy, SPA, CRUD handler, Provider request, OAuth operation, backup/restore operation, server configuration, or public route. |
| Matrix / references | `H01-H21`, `J02`, `J08-J09`, `J11-J15`, `J18-J20`; [ADR-0069](../adr/ADR-0069-versioned-management-openapi-contract.md); [BC-MGMT-002](../contracts/BC-MGMT-002-versioned-management-openapi.md); [OpenAPI contract](../openapi/management-v1.json) |

## Delivered contract

`management-v1.json` freezes the versioned `/admin/` resource surface for Config Versions,
Egress Policies, Upstreams, Endpoints, Credentials, bindings, Public Models, aliases, Routes,
Candidates, Access Groups, Client Keys, OAuth, Catalog, runtime status, audit and backup/restore.
Every operation has an operation ID and a bounded response shape. Future operations carry an
explicit owning P10 task rather than silently declaring an implementation live.

The document is `OpenAPI 3.1`, marked `contract_only`, has no `servers` declaration, and provides
neither a generic HTTP proxy nor a full YAML/JSON configuration upload. All paths are below
`/admin/`; public inference `/v1/*` endpoints are deliberately outside the document.

The root security scheme is the distinct `X-Management-Key`. Structured graph writes require both
`X-Config-Version` and an opaque `If-Match` revision. Credential input contains a write-only
secret, while Credential reads expose only `secret_present` and revision metadata. Ordinary
Client-Key reads have only an ID, group, prefix, status and expiry; the key itself occurs only in
the issuance response with an explicit `display_once` lifecycle marker. Errors have a bounded
code and message only.

## Schema and review correction

Review found that several output schemas extended closed input schemas with `allOf`. JSON Schema
would have rejected each output-only relationship field through the inherited
`additionalProperties: false`, making those composite responses unsatisfiable. The affected
Endpoint, binding, Alias, Route, Candidate, AccessGroupRoute and issued Client-Key schemas are
now independent closed output objects. A matching response component was also added for Route
Explain, so every local reference resolves.

The focused review confirms:

- Management credentials and inference Client Keys remain separate contracts.
- Secret values, Cookie/token aliases, encrypted payloads and Client-Key digests have no read or
  error-schema path; the one issuance value is explicitly display-once.
- Version selection and `If-Match` cover every structured concurrent graph write.
- The contract neither grants a management network listener nor pre-implements P10-02 admission,
  authentication, CSRF/CORS, UI or Provider operations.

## Verification

| Command / review | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --locked -p gateway-http-actix --test p10_01_management_openapi_contract` | PASS; six tests cover resource surface, local `$ref` resolution, independent Management Key, versioned graph writes, Secret/Client-Key lifecycle, closed output schemas and deferred-operation/listener boundary. |
| `cargo clippy --locked -p gateway-http-actix --test p10_01_management_openapi_contract -- -D warnings` | PASS |
| `git diff --check` | PASS |
| `./scripts/check.sh full` | PASS; plan state/guard, quality-tool cache, Rust format/Clippy/tests, source/crate policy, document links, tracked Secret scan, whitespace, dependency policy and RustSec audit all passed. All real-provider harnesses remained ignored. |

## Rollback and next task

Rollback removes only this contract-first material and its contract test; it does not alter active
Snapshots, public inference APIs, storage, Credential material, Client Keys, server state or
external traffic. P10-02 is the next task and may implement only the declared authentication,
network-admission, audit and CSRF/CORS boundary after its own plan/review cycle.
