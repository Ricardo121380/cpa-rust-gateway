# ADR-0081: Provider-owned account-pool facade

- Status: Accepted for P13-06A
- Date: 2026-08-11
- Scope: management read model and Provider runtime composition boundary

## Decision

CPAR exposes Provider-owned account-pool state through an injected, read-only facade. The facade
returns a single observed snapshot containing opaque Provider/Channel/Account identities,
scheduling metadata, authentication lifecycle, runtime availability, and maintenance deadlines.
The management handler validates filters and emits a bounded keyset page; it never contacts a
Provider or decrypts credential material.

`GET /admin/operations/account-pools` remains the selected Config Version's static
Endpoint-Credential binding inventory. Provider-owned live state is intentionally exposed by the
separate `GET /admin/operations/provider-account-pools` route so static configuration cannot be
reported as live Health, Quota, or Circuit state.

## Rationale

The repository already has immutable `EndpointCredentialPools`, concurrency leases, and exact-key
Health/Quota registries, while native Provider stores (for example Grok Build/Web/Console) own
their credential shape and lifecycle. A Provider-neutral facade lets those implementations be
adapted without making `gateway-control` depend on a specific Provider crate or creating a second
scheduler. The facade also gives the management UI a stable contract for later Provider adapters.

Authentication (`active`, `reauth_required`, `disabled`, `expired`) is not the same fact as runtime
availability (`available`, `cooling`, `circuit_open`, `quota_blocked`, `unauthorized`,
`recovery_in_flight`, `expired`); both are retained in the projection.

## Pagination and consistency

Rows are ordered by `(provider_id, channel_id, account_id)`. A cursor contains only the opaque
last key, the source `snapshot_id`, and a filter fingerprint. A cursor from another snapshot or
different filter set returns a safe `409`; it cannot silently mix observations.

## Security and non-goals

- No credential plaintext/ciphertext, endpoint URL, request body, Header/Cookie, or Client Key
  digest is returned.
- No Provider request, OAuth refresh, reauth, replenishment, proxy selection, or scheduler mutation
  occurs in this slice.
- Provider adapters retain independent credential, egress, quota, and failure domains; the facade
  never performs cross-Provider fallback or credential conversion.
- Automatic refresh/reauth is P13-12; generic Provider egress/proxy pools are P13-11; actual
  Grok/ChatGPT/Krill adapter injection is P13-06B.

## Consequences

The default management composition is fail-closed until an application injects a Provider
facade. Tests can use a validated in-memory snapshot facade, while production composition can
build a snapshot from existing native pools and registries without duplicating scheduling logic.
