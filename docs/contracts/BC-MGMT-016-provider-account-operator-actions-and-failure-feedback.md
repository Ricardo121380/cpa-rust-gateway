# BC-MGMT-016: Provider account operator actions and failure feedback

Status: local implementation complete; phase gate pending (`P13-06C`)

## Protected action contract

`POST /admin/operations/provider-account-pools/actions` accepts one exact
`provider_id/channel_id/account_id` target and one action:

- `cool_down`: `cooldown_ms` is required and bounded to a finite operator interval; an optional
  `upstream_model` narrows the Health key.
- `request_recovery`: `cooldown_ms` is absent; an optional `upstream_model` narrows quota recovery.

The request must pass the existing Management Key, network/origin admission, same-origin CSRF and
`X-Config-Version` binding. The target must exist in the current serving Provider account-pool
facade. The response is a no-store value-free receipt containing only the action state and safe
timestamps. Every accepted or rejected action is auditable without storing a request body or
credential material. No Config revision is incremented.

## Failure feedback contract

`GET /admin/operations/provider-account-pools/failures` accepts exact Provider/Channel/Account
filters, a bounded `limit`, and an opaque cursor. Results are ordered newest-first and contain:

- opaque provider/channel/account/request/attempt ids;
- terminal `ended_at_ms`;
- closed `GatewayError` `code` and `scope`;
- closed retry decision.

The source is the gateway-owned durable Attempt event stream. No upstream model, URL, HTTP header,
cookie, body, raw error text, Secret, token, quota window, or client-key digest is returned. The
reader is bounded; malformed or over-limit durable input returns a safe unavailable error instead
of a partial or guessed answer.

## Provider and lifecycle boundaries

Provider identity is exact. A Grok Build failure cannot affect Grok Console/Web, Codex or Krill;
no action performs cross-Provider fallback or credential conversion. Existing Config Version
credential/binding enablement remains the path for ordinary durable configuration. Automatic
refresh/reauth/replenishment and Provider-specific native-store mutations are outside this
contract and require P13-12 or a new change request.

## Frontend handoff

`docs/openapi/management-v1.json` is the authoritative contract. Codex has run
`npm --prefix web/prism run sync-contract`; Claude Code must consume the generated methods and
decide the UI placement. The backend does not hand-edit Prism application code in this slice.
