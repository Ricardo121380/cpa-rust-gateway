# P10-05 Protected routing and Client Key workflows

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P10-05` |
| Date | `2026-07-24` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` — implementation, targeted verification, browser E2E, documentation/Secret checks and independent scope/security review passed. The single P10 Full preflight and Delivery Gate remain Phase work. |
| Scope / Task Card | Public Model, Alias, Route, Candidate, Access Group, grant and Client Key draft workflows; no runtime, publication, backup/restore, serving or Provider traffic. |
| Matrix / references | `H03`、`H05-H06`、`H18`、`H21`、`L17-L20`、`L27-L29`、`L35`; [ADR-0073](../adr/ADR-0073-protected-routing-client-key-workflows.md); [BC-MGMT-006](../contracts/BC-MGMT-006-protected-routing-client-key-workflows.md). |

## Delivered behavior

P10-05 extends the P10-02-protected management Scope with draft-only routing and access-resource
operations. Every mutation uses the existing exact `If-Match` Version transaction and resource
audit append. The protected HTTP integration creates the complete synthetic `minimax-m3` graph,
checks ETag progression and stale-write rejection, then proves Client Key responses are redacted
after issuance and that schema-owned delete cascades remove grants, candidates and keys.

Client Key issuance receives an explicit `ClientKeyService` dependency. It writes only Prefix and
HMAC digest storage and returns a complete Key after commit in the one issue result. All normal
metadata endpoints omit it. The UI puts that immediate value in a separate `Display once` pane;
a subsequent operation, failure, explicit clear or reload removes it.

Route validation is local topology validation only. P10-05 neither asks the Router to choose a
Candidate nor accesses a Provider/runtime handle. The response boundary rejects legacy stored
Route policies rather than reporting an incorrect `smooth_weighted_round_robin` policy.

## Local browser evidence

`scripts/p10-05-browser-fixture.mjs` serves only the compiled static assets at loopback and
synthetic value-free management responses. No real account, Provider route, credential, proxy or
external network path is read or contacted.

| Browser action | Observed result |
|---|---|
| Connect synthetic Management Key/CSRF | Page reports `Connected in memory`; a reload returns `Not connected`. |
| Create Public Model, Alias, Route, Candidate, Access Group and grant | The fixture accepted only the expected contract body shapes; ETag advanced from `rev-0` through `rev-6`. |
| Issue one synthetic Client Key | ETag advanced to `rev-7`; the normal result contained only redacted metadata and the fixture-only display value appeared solely in `Display once`. |
| Read the Client Key after issue | The display-once pane was cleared before the GET; the `200` metadata result contained no full Key. |
| Reload | Management Key/CSRF inputs, in-memory client, revision state and Client Key pane were reset. |

The static SPA checker separately rejects browser-storage, Cookie, clipboard and direct-fetch
paths and verifies the same OpenAPI-generated client owns every P10-05 operation.

## Targeted verification

| Command / review | Result |
|---|---|
| `cargo fmt --all -- --check` and targeted three-crate Clippy | PASS with `-D warnings`; public error docs, typed Key issue/update input and route registration were also reviewed. |
| `cargo test --locked -p gateway-store control_plane --lib` | PASS; 7 tests. |
| `cargo test --locked -p gateway-control management_mutation_service --lib` | PASS; 2 tests. |
| `cargo test --locked -p gateway-http-actix --test p10_05_management_routing` | PASS; protected graph, redaction and cascade E2E. |
| `cd web/admin-ui && npm run check` | PASS; 65 generated operations and reproducible static build. |
| Local browser fixture + Playwright | PASS; seven synthetic mutations advanced `rev-0` to `rev-7`, immediate-only Key display was cleared by GET/reload, browser storage was empty and reload disconnected. No console error remained after the fixture supplied a CSP response header with `frame-ancestors 'none'`. |
| `git diff --check`, tracked Secret scan and `./scripts/check.sh docs` | PASS; 284 Markdown files and the plan-state check accepted zero `IN_PROGRESS` tasks at closeout. |
| Independent scope/security review | PASS: no runtime Router/Provider handle, egress path, publication, backup/restore, browser persistence, Cookie, clipboard or Client Key re-presentation was introduced. The final static embedded host must carry the reviewed CSP `frame-ancestors` response header in P10-09. |

## Remaining closeout

P10-05 is closed locally. P10-06 may use this evidence but remains the first owner of runtime
Health/Quota/403, Route Explain and tracing pages. P10-07, P10-08 and P10-09 remain excluded.
