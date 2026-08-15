# P13-06A Provider account-pool facade report

## Scope

P13-06A establishes the Provider-owned account-pool management boundary. It is intentionally a
read-only snapshot/facade slice; Provider adapters, automatic refresh/reauth, proxy pools, and
production changes remain later tasks.

## Implemented

- `gateway-control::provider_account_pool_service` validates and sorts bounded Provider-owned
  rows, keeps authentication and runtime states independent, and binds keyset cursors to both a
  snapshot and the exact filter fingerprint.
- `GET /admin/operations/provider-account-pools` is protected by the existing Management Key and
  management admission scope. It returns `no-store` pages and safe error envelopes.
- OpenAPI and generated TypeScript management client include closed schemas, single-value filters,
  enum bounds, and the P13-06A delivery label.
- The HTTP fixture covers exact Provider filtering, two-page pagination, status separation,
  stale/filter-conflicting cursors, missing management credentials, and absence of secret/URL/body
  fields. Existing static Config Version inventory tests remain unchanged.

## Evidence

The following local checks are required for this slice:

```text
cargo test --locked -p gateway-control provider_account_pool
cargo test --locked -p gateway-http-actix --test p13_04_management_inventory
node scripts/generate-management-client.mjs --check
jq empty docs/openapi/management-v1.json
```

The final review also runs workspace formatting, strict Clippy for the touched crates, the
management OpenAPI contract test, SPA checks, documentation checks, and `git diff --check`.

Final local result: gateway-control 53/53, gateway-http-actix lib 54/54, management OpenAPI 7/7,
management inventory HTTP fixture 1/1, generated-client/SPA reproducibility, strict Clippy, fmt,
documentation links/contract references/plan state, tracked secret scan, and diff whitespace all
passed.

## Boundary and next step

No Provider request, credential decryption, scheduler mutation, production deployment, or GitHub
Delivery Gate was performed. The default source fails closed until an application injects a
Provider facade. P13-06B should adapt existing Grok/ChatGPT/Krill native/runtime pools into this
facade and verify lease/Health/Quota/Circuit consistency without introducing cross-Provider
fallback or automatic reauth.

Status: `DONE_WITH_BOUNDARY` after formal P13 Delivery Gate run
[31858904767](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31858904767).
