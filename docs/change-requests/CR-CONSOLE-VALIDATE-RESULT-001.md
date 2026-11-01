# Validation reports an invalid graph as 409 without reasons

Status: open — observed by the frontend (Claude Code) while verifying `web/console` against a real gateway.

## Contract vs. implementation

`docs/openapi/management-v1.json` describes `POST /admin/config-versions/{config_version_id}/validate`
as `200 Validation { valid: boolean, error_codes?: string[] }`.

`crates/gateway-http-actix/src/management_lifecycle_resources.rs::validate_version` only ever answers
`200 { "valid": true, "error_codes": [] }`. Any failure of `validate_configuration` (compile, snapshot
build, egress policy, client-key material …) is mapped by `ManagementServiceError::lifecycle_failure`
to `409 management_lifecycle_conflict` — "Management lifecycle operation is not available for the
current configuration". The code is not named in the contract, and the response carries no reason.

Reproduced on a fresh gateway: a draft containing one `openai-compatible` Upstream with no Endpoint
validates as 409; the same draft after deleting that Upstream validates as 200.

## Requested change

Return `200 { "valid": false, "error_codes": [...] }` with stable, value-free codes for validation
failures (e.g. `upstream_without_endpoint`), keeping 409 for real lifecycle conflicts (wrong status,
revision). If 409 stays the answer, please name `management_lifecycle_conflict` in the contract.

## Frontend meanwhile

`web/console` (`validateVersion` in `src/api/hooks.ts`) maps this 409 to `{ valid: false,
error_codes: ["management_lifecycle_conflict"] }`, blocks publishing, and tells the operator that the
gateway gave no reason.
