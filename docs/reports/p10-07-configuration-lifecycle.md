# P10-07 Configuration lifecycle report

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P10-07` |
| Date | `2026-07-24` |
| Branch | `codex/p10-control-plane` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Scope | Protected Config Version metadata, read-only validation, P2 publication, retained-predecessor rollback, and P2 lifecycle-audit projection. |

## Delivered boundary

The P10-07 adapter adds protected `/admin/config-versions` and `/admin/audit-events` routes over
the existing P2 `ManagementService`; it does not duplicate P2's transaction and Snapshot
publication implementation. List/get use a root-only Config Version metadata projection, so they
do not load resource graphs, Credential envelopes/ciphertext, Client Key data, URLs, or request
material. The adapter maps lifecycle failures to closed `409` or `503` management responses only.

Publication accepts only a `draft` Version and requires its frozen `If-Match` revision. Rollback
selects no caller-supplied archived Version; it invokes P2's one retained predecessor operation.
The lifecycle-audit projection exposes only monotonic ID, closed action, bounded actor/time,
target Version, and optional replaced Version. Backup, restore, Provider transport, endpoint
probes, Catalog work, persistence/export controls, proxy/TLS input, and external egress remain
outside this task.

The SPA uses generated-client operations only: list/get/create/validate/publish/rollback/audit.
Its management and CSRF inputs remain page-local and are cleared on reload. It includes no P10-08
backup or restore action.

## Verification

| Evidence | Result |
|---|---|
| `cargo test --locked -p gateway-store control_plane::tests::malformed_persisted_crypto_records_fail_closed` | PASS |
| `cargo test --locked -p gateway-http-actix --test p10_07_management_lifecycle` | PASS — 2 tests: protected lifecycle invariants and default fail-closed facade. |
| `cargo clippy --locked -p gateway-store -p gateway-control -p gateway-http-actix --tests -- -D warnings` | PASS |
| `npm --prefix web/admin-ui run check` | PASS — 65 generated operations and reproducible static build. |
| `./scripts/check.sh docs` | PASS — plan state, links, tracked Secret scan, and whitespace. |
| `cargo fmt --all -- --check`, `git diff --check` | PASS |

## Browser E2E

The loopback-only fixture at `127.0.0.1:4182` served static assets and deterministic, non-secret
lifecycle metadata only. It had no database, Provider transport, backup material, proxy, or
external egress. The browser flow confirmed:

1. An absent-Version publish returns the safe lifecycle `409` response.
2. Version 1 is created, validated, and published.
3. Version 2 is created and published, replacing Version 1.
4. Rollback restores only Version 1 and replaces Version 2.
5. The audit view returns five ordered rows: create, publish, create, publish, rollback.
6. Cookie, `localStorage`, and `sessionStorage` are empty; reload returns to `Not connected`.

## Review

Focused review found no remaining release-blocking issue. It checked P2 compile/SQLite/audit/
Snapshot ordering, stale-revision handling, draft-only publication, retained-predecessor rollback,
metadata-only reads, response redaction, unavailable-facade behavior, generated-client-only UI
calls, and the P10-08 exclusion. The one stale UI label claiming publication was unavailable was
corrected before verification.

P10's single GitHub Delivery Gate remains deferred until all P10 tasks are locally reviewed and
committed; this task does not push, deploy, or start P10-08.
