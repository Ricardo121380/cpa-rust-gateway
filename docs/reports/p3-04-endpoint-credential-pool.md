# P3-04 Endpoint Credential pool report

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P3-04` |
| Matrix / behavior | `D12`, `D14`, `D17`, `E16`, `K06`, `L26`; Behavior 3/14/17/20 |
| Date | `2026-07-20` |
| Branch | `codex/p3-04-credential-pool` |
| Rust | `1.97.1` |
| Result | PASS locally after review; GitHub Fast/Full acceptance pending |

## Delivered scope

- Added `gateway-upstream::EndpointCredentialPool` and `EndpointCredentialPools`: immutable,
  Endpoint-local Credential entries, lower-is-better priority tiers, deterministic bounded smooth
  weighted plans, and independent atomic cursors. Each tier is capped at `1024` slots.
- Added non-cloneable `CredentialLease` values. Each successful per-Credential CAS reservation is
  released by `Drop` or consuming `release(self)`, so cancellation releases capacity without a
  global request-path lock.
- Added redacted, zeroizing `CredentialSecret` material and
  `gateway-control::CredentialPoolCompiler`. It validates complete Endpoint/Credential/Upstream
  relations, authenticates the existing AAD before decryption, rejects malformed inactive relations
  and duplicate bindings, and returns no partial pool set.
- Added `gateway-router::RouteCredentialScheduler`, which composes the P3-03 Candidate scheduler
  with selected Endpoint pools. Candidate and Credential cursor sets stay independent, preserving
  Route weights even when an Endpoint has multiple Credentials.
- Added [ADR-0014](../adr/ADR-0014-endpoint-credential-pool-leases.md) and
  [BC-CRED-001](../contracts/BC-CRED-001-endpoint-credential-pool-leases.md), with synthetic
  tests for weighting, saturation, cancellation/release, concurrency, AAD failure, structural
  rejection, redaction, and two-layer fairness.

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-control -p gateway-router -p gateway-upstream` | PASS; 27 control, 17 router, and 26 upstream tests, including pool, structural rejection, and two-layer fairness coverage |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` | PASS |
| `ruby scripts/check-crate-boundaries.rb`, `ruby scripts/check-source-policy.rb`, and `ruby scripts/check-doc-links.rb` | PASS; 21 package boundaries, 52 Rust files, and 85 Markdown documents |
| `scripts/secret-scan.sh --staged` and `git diff --cached --check` | PASS; all new files were included in the staged Secret and whitespace checks |
| `cargo deny check` and `cargo audit` | PASS; existing duplicate-version notices are policy-allowed warnings |
| `./scripts/check.sh fast` | PASS |
| `./scripts/check.sh full` | PASS |

## Review

Review passed. It verified that the pool keeps Endpoint, tier, and Credential contention local:
the precompiled tier schedule bounds each scan to at most `1024` slots, each tier has its own
atomic cursor, and CAS cannot reserve more than the configured concurrency. A lease owns exactly
one successful reservation; its non-cloneable RAII form means explicit release and cancellation
both return capacity once.

The review verified that plaintext remains in a zeroizing wrapper, every public `Debug` path is
redacted, AEAD/AAD authentication happens only on the control path, and request-time selection has
no Repository, `SecretStore`, SQLite query, or global scheduler lock. It also verified Candidate
selection happens before Endpoint Credential selection, so the concurrent `3:1` Route and `1:1`
Endpoint test demonstrates two independent weighting layers. Missing pools, saturation, unknown
Routes, and predicate rejection all yield the safe `CredentialUnavailable/Credential` result.

Review found and corrected a complete-graph validation gap before local acceptance: an unused
disabled Endpoint or Cooling Credential with an orphaned Upstream, and a duplicated
Endpoint/Credential binding, could previously evade this compiler's direct validation. The compiler
now rejects those states before decryption or pool return, with regression coverage. The matching
Snapshot/pool configuration-generation invariant remains an explicit P3-04 construction
precondition; a publication/runtime holder is deferred to P3-06 rather than adding transport,
health, or attempt behavior here.

## Scope and deferred work

P3-04 does not persist per-request lease rows, modify Credential revision/state, create a health or
Circuit model, classify 401/403/429/5xx, invoke a Provider, build/send HTTP, retry/fail over,
parse responses, emit events, or contact a deployed Endpoint. P3-05 owns dynamic
health/cooldown/circuit state; P3-06 owns attempts, exclusion, retry, and first-semantic-event
behavior. All current fixtures use synthetic secrets and endpoints only.

## GitHub CI

The implementation commit's GitHub Fast and Full gates must both pass before the P3-04 acceptance
record is finalized. Its separate verification-record and final status commits must also pass the
same workflow before P3-05 can begin.
