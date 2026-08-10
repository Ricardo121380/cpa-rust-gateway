# P13-04A Provider-aware management inventory report

| Field | Value |
|---|---|
| Plan version | `v1.244` |
| Task | `P13-04A` |
| Date | `2026-08-11` |
| Scope | Secret-free, read-only Provider/Channel/Account/Pool configuration inventory |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |

## Intended delivery

P13-04A introduces the first CPAMP-like operations read model without creating a second control
plane. `GET /admin/operations/account-pools` will project one row per Endpoint-Credential binding
from the selected Config Version. It will provide stable Provider, Channel, Account, binding and
route relationship metadata for a frontend or operator tool, while retaining the existing
Management Key, Config Version, revision/ETag, CSRF and AEAD boundaries.

The initial projection deliberately excludes endpoint URLs and paths, credential ciphertext and
plaintext, key digests, request bodies and all live Provider observations. It therefore describes
configured topology and eligibility only. Native Provider-owned pools, Health, Quota, Circuit,
usage, cost, billing, refresh and reauth remain later phase work.

## Acceptance matrix

| Area | Required evidence | Current result |
|---|---|---|
| Typed projection | Provider/Channel/Account/Binding/route fields map from one Config Version | `PASS_LOCAL` |
| Filtering | Provider, Channel, persisted account status and static enabled conjunction | `PASS_LOCAL` |
| Pagination | Stable keyset `(provider_id, channel_id, account_id)`, default 50, max 100 | `PASS_LOCAL` |
| Cursor safety | Version/revision-bound cursor; stale or cross-version cursor returns `409` | `PASS_LOCAL` |
| Secret safety | No URL/path, ciphertext/plaintext, digest, headers or request body in JSON | `PASS_LOCAL` |
| Management admission | Existing Management Key, selected Config Version and ETag/CSRF behavior | `PASS_LOCAL` |
| Provider isolation | No Provider call, lease, Snapshot publication or production mutation | `PASS_LOCAL` |
| Contract/client | OpenAPI closed schemas and generated TypeScript operation | `PASS_LOCAL` |
| Review/gates | Focused unit/HTTP review, local gates, then one P13 Delivery Gate at phase close | `LOCAL_PASS_PENDING_PHASE_GATE` |

## Explicit non-claims

An `enabled` item means `provider_enabled && channel_enabled && binding_enabled`; it does not mean
that the Credential is active, healthy, under quota, or currently routable. The inventory does not
include native Grok runtime accounts unless a later Provider-specific facade is deliberately added.
No live upstream request is authorized or implied by this report.

## Rollback and follow-up

The slice is reversible by removing its projection module, protected route, OpenAPI/client
operation and tests. Existing P10 resources, Config Versions, active routes, Provider pools and
production listeners remain unchanged. After P13-04A implementation and review, the remaining
P13-04 operations read-model slices can be sequenced before P13-05 Usage/Quota/Cost/Billing and
P13-06 Provider-aware runtime account pools.

## Evidence log

Local evidence completed on 2026-08-11:

- `cargo test --locked -p gateway-control management_operations` — 3 tests passed;
- `cargo test --locked -p gateway-http-actix --test p13_04_management_inventory --test p10_01_management_openapi_contract` — 8 tests passed;
- `cargo clippy --locked -p gateway-control -p gateway-http-actix --all-targets --all-features -- -D warnings` — passed;
- `npm --prefix web/admin-ui run check` — 71 generated operations, reproducible static build;
- `./scripts/check.sh docs` — document links, contract references, plan state and secret scan passed;
- `git diff --check` and `cargo fmt --all -- --check` — passed.

No Provider request, production/server mutation, or GitHub Delivery Gate was performed. The phase
Gate remains pending by design; this report is not evidence of a formal P13 release.
