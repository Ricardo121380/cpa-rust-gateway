# P12-10I-10 Review

Decision: `PASS_WITH_EXTERNAL_BLOCKERS`

## Reviewed change

`apps/gateway/src/grok_admin.rs` no longer treats a batch-scoped native probe as valid only when
the entire Provider pool has one account. The probe still selects exactly one enabled account by
batch, compiles the normal pool, seeds the normal Health/Quota state, and acquires a lease through
a non-secret Credential-ID predicate restricted to that selected account.

This is a diagnostic/probe-scope correction only. It does not alter request scheduling, provider
fallback, route publication, account metadata, credential storage, or production configuration.

## Findings

- **No P0/P1 finding.** The old whole-pool cardinality check could produce a false `ProbeRejected`
  when another account was present, but it failed closed before any upstream request.
- **Build remains externally unverified after the correction.** The fixed artifact must be rebuilt
  and used for one fresh isolated SSO-derived native probe.
- **Console remains externally blocked.** The SSO envelope imported safely, but the native probe
  reached the provider boundary and was classified as `EgressRejected/egress`.
- **Web remains policy-blocked.** Cookie shape passed after seconds-to-milliseconds normalization,
  while the source expiry exceeded the strict 90-day CPAR session lifetime. No expiry was clamped.

## Evidence reviewed

- Autoreg SSO task shape and conversion path were inspected value-free.
- SSO→Build conversion and CPAR encrypted import returned `PASS`.
- Console and Web outcomes were recorded with fixed categories and no secret material.
- `cargo fmt --all -- --check` passed.
- `cargo test --locked -p gateway` passed.
- `cargo test --locked -p gateway-upstream credential_pool` passed, including the existing
  eligibility-predicate lease regression.
- `cargo clippy --locked -p gateway -- -D warnings` passed.
- The active graph, production listeners, and public route were not changed.

## Review conclusion

The local correction is appropriately narrow and fail-closed. P12-10I-10 cannot be promoted to a
successful live-provider matrix until the rebuilt artifact passes the SSO-derived Build probe and
the separately blocked Console/Web boundaries are resolved under their own approved changes.

P12-10I-11 later passed the exact-artifact SSO-derived Build probe in a two-account isolated pool.
That closes the Build finding; Console and Web retain their independent external blockers.
