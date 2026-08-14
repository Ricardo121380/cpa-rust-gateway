# BC-MGMT-014: Provider-owned account-pool inventory

## Contract status

Accepted for P13-06A local implementation; phase Delivery Gate is pending.

## Endpoint

`GET /admin/operations/provider-account-pools`

The endpoint is mounted only under the existing protected management scope. It accepts exact
single-value filters `provider_id`, `channel_id`, `auth_status`, `runtime_status`, `enabled`, a
bounded `limit` (default 50, maximum 100), and an opaque cursor.

## Response

The response contains `snapshot_id`, `observed_at_ms`, `items`, and `next_cursor`. Each item
contains only:

- opaque `provider_id`, `channel_id`, and `account_id`;
- bounded `account_kind`;
- independent `auth_status` and `runtime_status` enums;
- enabled, priority, weight, maximum concurrency, active leases;
- optional expiry, refresh-due, and quota-sync-due timestamps.

There is no URL, secret, ciphertext, request body, Header/Cookie, or Client Key digest.

## Snapshot and cursor rules

The Provider adapter supplies one validated snapshot. Rows use stable
`(provider_id, channel_id, account_id)` ordering. A cursor is valid only for the same snapshot and
the same filter fingerprint. Snapshot or filter drift returns `409`; invalid query/cursor data
returns the standard safe `400` management error. Successful responses are `Cache-Control: no-store`.

## Isolation rules

Provider-specific adapters own credential parsing, egress/session binding, quota observation,
health/circuit transitions, and refresh policy. The management facade is read-only and Provider
neutral. It does not lease an account, call an upstream, refresh OAuth, start Autoreg, create a
proxy pool, or fall back to another Provider. The existing Config Version account-pool endpoint
continues to represent static bindings only.

## Verification

The local contract is verified by control unit tests for sorting, exact filtering, invalid bounds,
status separation, cursor conflicts, and duplicate/invalid rows, plus a protected Actix fixture
covering management authentication, filters, pagination, no-store, stale cursor, and secret-free
serialization. OpenAPI and generated TypeScript client checks must pass before the P13 phase gate.
