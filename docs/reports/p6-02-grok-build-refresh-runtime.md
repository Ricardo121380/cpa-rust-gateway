# P6-02 Grok Build refresh singleflight and durable revision runtime report

| Field | Value |
|---|---|
| Plan version | `v1.9` |
| Task | `P6-02` |
| Date | `2026-07-22` |
| Branch | `codex/p6-grok-build` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Scope / budget | `M`; local sealed OAuth runtime only; no real Provider traffic, test account, or server change |
| Execution channel | Current default model, `medium`; this task needs persistence and concurrency review. Luna is unavailable in this execution surface; no subagent was used. |
| References | Matrix `E25`、`E26`、`E29`; [ADR-0043](../adr/ADR-0043-grok-build-refresh-runtime.md); [BC-CRED-004](../contracts/BC-CRED-004-grok-build-refresh-runtime.md) |

## Delivered behavior

P6-02 adds `grok_build_credential_runtime`, keyed by exact, non-blank, at-most-128-byte Config
Version/Credential identity. It stores a bounded Grok Build OAuth Credential only as AEAD
ciphertext using the existing versioned Master Key ring. Associated data binds the ciphertext to the exact identity, and both
the persisted plaintext encoder and recovered values remain redacted/zeroizing.

The runtime state starts at revision zero and uses atomic compare-and-swap to write a later
Credential. The refresh coordinator is keyed per Credential, has no global refresh lock, and
waits a bounded 30 seconds by default for an active same-key leader. A late refresh response cannot
overwrite an external/newer revision. A fresh external winner is returned as `Superseded`; an
expired one is an explicit retry state rather than a false transport error.

All test values are synthetic. The injected transport blocks only inside the test process; it does
not create a socket, inspect ambient configuration, contact xAI, alter a server, or use a real
Credential.

## Verification

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p6_02_refresh_runtime` | PASS; 8 synthetic persistence, CAS, same-key singleflight, distinct-Credential concurrent refresh, stale-winner, timeout, and identity-bound tests passed. |
| `cargo clippy --locked -p provider-grok --all-targets --all-features -- -D warnings` | PASS after correcting strict documentation and API-shape findings. |
| `cargo fmt --all -- --check` and `git diff --check` | PASS. |
| `ruby scripts/check-crate-boundaries.rb` | PASS; narrow P6-02 `gateway-store`/`rusqlite` edge only. |
| `./scripts/check.sh full` | PASS; all local plan/CI/script/format/Clippy/test/docs/dependency/supply-chain checks passed. |
| Staged Secret scan and independent code review | PASS; staged scan found no credential pattern, and review corrected bounded identity admission, wait/error semantics, and conflict reporting before commit. |

## Review focus and next task

The implementation review explicitly corrected four unsafe ambiguities before this report: a
same-key wait cannot be unbounded; an expired external CAS winner must not be reported as a local
transport failure; runtime identities must be bounded before AAD allocation; and a CAS-conflict
diagnostic must not claim a revision that a concurrent writer can change immediately afterward.
The test suite proves the operational outcomes, restart recovery, identity admission, and that the
stored ciphertext does not contain the synthetic plaintext token.

P6-02 does not construct Build inference HTTP, does not classify Provider errors, and does not
write quota, cache, response-continuity, health, or authorization state. It is now
`LOCAL_PASS_PENDING_PHASE_GATE`; P6-03 is the sole `IN_PROGRESS` task for fixed-fixture Build
Responses request/stream/error implementation. G6 remains P6's only remote Delivery Gate.
