# P13-04 Management operations closeout report

| Field | Value |
|---|---|
| Plan version | `v1.245` |
| Task | `P13-04` (`P13-04A` + `P13-04B`) |
| Date | `2026-08-11` |
| Scope | CPAMP-like typed management backend foundations |
| Status | `DONE_WITH_BOUNDARY` |

## Delivered surface

P13-04 now provides two protected, backend-first operational read models:

- `GET /admin/operations/account-pools`: selected Config Version, one row per
  Endpoint-Credential binding, Provider/Channel/Account/Binding/Route metadata, static enabled
  filters and revision-bound keyset pagination;
- `GET /admin/operations/usage`: bounded durable Request/Attempt/Usage aggregation grouped by
  Provider/Channel/Account/public model/protocol/Client Key/Access Group, token confidence and
  explicitly unpriced cost.

The existing P10 resources remain authoritative for Management Key admission, Config Version
revision/ETag, audit events and live runtime Health/Quota/availability. No duplicate control plane
or implicit cross-Provider fallback was introduced.

## Acceptance matrix

| Area | Evidence | Result |
|---|---|---|
| Config inventory | P13-04A typed compiler and protected HTTP fixture | `PASS_LOCAL` |
| Usage lineage | Request + highest Attempt + final Usage join; failed/missing lineage fails closed | `PASS_LOCAL` |
| Usage grouping | Provider/Channel/Account/model/protocol/client/access-group deterministic grouping | `PASS_LOCAL` |
| Filters and time window | Identity, model, protocol and inclusive Attempt-end-time filters | `PASS_LOCAL` |
| Pagination | Default 50/max 100 stable keyset cursor | `PASS_LOCAL` |
| Token confidence | Exact/partial/unknown with checked totals | `PASS_LOCAL` |
| Cost safety | Null cost with `unpriced`; no hard-coded price | `PASS_LOCAL` |
| Runtime quota boundary | Existing P10 runtime availability/quota projection reused; no live claim in usage read model | `PASS_LOCAL` |
| Production source | Read-only SQLite event facade with bounded scan; no Provider or configuration mutation | `PASS_LOCAL` |
| Contract/client | OpenAPI closed schemas and generated TypeScript operations | `PASS_LOCAL` |
| Review/gates | Focused tests, Clippy, SPA/client, docs/diff checks and formal P13 Delivery Gate run 31858904767 | `PASS` |

## Evidence and boundary

The formal P13 Delivery Gate run 31858904767 passed for the exact pushed revision. This report
closes P13-04's backend scope with an explicit boundary: no production deployment, server
mutation, Provider request, OAuth refresh, account-pool lease or public UI work was performed.

Rollback removes the P13-04B route/facade/compiler/OpenAPI/client and documentation. The event
writer, existing P10 management resources, P10 runtime quota state and P13-04A inventory remain
independently reversible.
