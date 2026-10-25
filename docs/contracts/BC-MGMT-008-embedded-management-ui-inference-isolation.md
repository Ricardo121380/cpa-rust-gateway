# BC-MGMT-008 API-only management and inference-route isolation

| Field | Value |
|---|---|
| Contract | `BC-MGMT-008` |
| Status | Accepted |
| Decision | [ADR-0124](../adr/ADR-0124-cpa-first-backend-and-frontend-retirement.md) |
| Domain | Public inference and protected management listener separation |

## Behavior

The public listener contains health and inference routes, and exposes no management API.
The management listener contains the login API and protected `/admin` routes; it exposes no
public health/model/inference route. BC-MGMT-003 continues to govern its admission.

The old frontend is removed. `/admin-ui`, `/admin-ui/` and its old assets return 404 on both
listeners. Building the HTTP crate requires neither a frontend source tree nor Node/npm.

## Corresponding tests

- `public_listener_does_not_mount_management_routes_or_ui`
- `management_listener_does_not_mount_public_or_retired_ui_routes`

The local process smoke in `scripts/test-p12-02-serve.sh` also checks actual listener separation,
retired UI rejection, authenticated configuration reads and backup preflight.
The original P10-09 embedding decision and results remain in historical Git and reports.
