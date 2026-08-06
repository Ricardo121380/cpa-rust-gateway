# P12-10I-16 grok2api v3.1.1 native port receipt

## Scope

Compared upstream `chenyme/grok2api` tag `v3.1.1` with CPAR and ported the provider behavior that fits the existing Rust boundaries. No grok2api runtime dependency, server change, credential read, or production graph change was made.

## Implemented

- Native P-256 DPoP session, JWK thumbprint validation, proof signing, bounded cache, and matching-token invalidation.
- Console transport performs the DPoP token exchange, applies `Authorization: DPoP` plus proof, and retries one matching `401` after session invalidation.
- Console usage projection validates chat/image/video windows and predicts a bounded 24-hour chat recovery window when exhausted.
- Migration accepts the v3.1.1 top-level JSON-array credential export through the same atomic CPAR transaction as NDJSON.

## Validation

- `cargo test -p provider-grok`: PASS.
- `cargo clippy -p provider-grok --all-targets -- -D warnings`: PASS.
- Dedicated migration, DPoP, quota, and Console runtime tests: PASS.
- No live upstream request was sent.

## Deferred

Console image/video/media routes are not claimed complete. CPAR currently has no unified media Canonical protocol or HTTP route contract; that is a separate typed protocol slice before upstream media endpoints can be safely wired.
