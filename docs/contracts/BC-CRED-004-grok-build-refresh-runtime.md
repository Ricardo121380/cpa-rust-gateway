# BC-CRED-004 Grok Build refresh singleflight and durable revision runtime

| Field | Value |
|---|---|
| Contract | `BC-CRED-004` |
| Task | `P6-02` |
| ADR | [ADR-0043](../adr/ADR-0043-grok-build-refresh-runtime.md) |
| Status | `DONE` |
| Domain | Per-Credential OAuth refresh, AEAD persistence, and stale-write prevention |

## Entry and retained state

`GrokBuildCredentialRefreshCoordinator` receives an exact `GrokBuildCredentialKey` comprising a
non-blank, at-most-128-byte Config Version ID and Credential ID, a durable state port, the pure
P6-01 OAuth flow, an injected transport, and a caller-supplied instant.
`GrokBuildCredentialSqliteStore` stores only this key, revision, AEAD ciphertext, Master Key
version, and update time in
`grok_build_credential_runtime`.

The table is runtime state rather than configuration. It does not publish a Route, alter a
`RouteSnapshot`, or change `upstream_credentials`. It intentionally has no configuration foreign
key or implicit cleanup cascade; a caller must derive the exact identity from a validated compiled
Config Version and a later lifecycle task must own historical-state cleanup.

## Invariants

| Concern | Required behavior |
|---|---|
| Exact identity | Both identity components are non-blank and at most 128 bytes before any AAD/SQLite work. Ciphertext associated data binds the P6 domain plus their length-prefixed values. A row copied to another identity fails closed. |
| Secret persistence | Access/refresh tokens use the existing AEAD `SecretStore`; plaintext is bounded/zeroizing and never appears in `Debug`, persistence error text, or test output. |
| Initial state | `insert_if_absent` writes revision `0` only when no exact row exists. A concurrent/existing row is loaded, never replaced. |
| Conditional refresh | A successful refresh can commit only with its observed revision; the durable revision then increases by exactly one. Missing and conflict are explicit outcomes. |
| Stale result | A CAS loser reloads the winner. If it is usable it returns `Superseded`; if the winner is still expired it returns `ConcurrentCredentialStateChanged` and does not use or overwrite it. |
| Singleflight scope | Only the same exact key shares one flight. Separate Credentials are not held behind a Provider-global lock. |
| Wait bound | Same-key followers wait at most the configured positive duration (30 seconds by default). On expiry they return `RefreshLockTimedOut` without opening a second refresh request. |
| Failure scope | Persistence/OAuth/coordination failures do not mutate Credential authorization, quota, health, routing, or account state. P6-07 owns Provider error classification. |

## Required result sequence

1. A current Credential returns `Current` without invoking refresh transport.
2. The first caller observing expiry becomes leader, exchanges exactly one pure OAuth refresh, then
   CAS-commits `Refreshed(revision + 1)` only if its observed revision still matches.
3. Same-key concurrent callers wait. They receive the same safe leader error, or reload the new
   durable current state after leader success; they must not independently refresh while the leader
   is active.
4. If an external writer wins while the leader is in transport, the leader never overwrites it. A
   fresh winner returns `Superseded`; an expired winner returns an explicit retry state.

## Failure semantics

| Condition | Result |
|---|---|
| Missing exact state | `MissingCredentialState`; no new row is guessed. |
| Bad revision/envelope/plaintext | `InvalidPersistedState` or `SecretStoreFailure`; no partial Credential is returned. |
| Store lock/SQLite unavailable | `StoreUnavailable` wrapped as a safe refresh persistence error. |
| Revision overflow | `RevisionOverflow`; no update is written. |
| OAuth protocol/mock transport failure | P6-01 `GrokBuildOAuthError`; no retry loop is created by the coordinator for followers. |
| Same-key leader still active after wait bound | `RefreshLockTimedOut`; no second transport call and no account penalty. |
| CAS winner remains expired | `ConcurrentCredentialStateChanged`; caller reloads/retries through the coordinator instead of treating it as transport loss. |

## Deferred behavior

P6-03 owns real Build HTTP/TLS, egress admission, leader network deadlines, Responses streaming,
and authorized test-account verification. P6-04 owns Billing/Quota Window persistence and reset
semantics. P6-05/P6-06 own cache identity, affinity, response ownership, and reasoning replay.
P6-07 owns raw 401/403/429/transient classification and any revision-guarded destructive state
transition. This contract sends no real Provider traffic and changes no server.

## Corresponding tests

- `sqlite_runtime_state_is_aead_sealed_revisioned_and_recovers_after_reopen`
- `concurrent_expiry_starts_one_refresh_and_all_callers_observe_the_new_revision`
- `distinct_credentials_refresh_independently_under_concurrent_expiry`
- `stale_refresh_result_cannot_overwrite_an_external_newer_revision`
- `expired_external_cas_winner_is_a_safe_retry_state_not_a_transport_failure`
- `waiter_times_out_without_starting_a_second_refresh`
- `runtime_identity_rejects_blank_or_oversized_components_before_aad_or_sqlite`
