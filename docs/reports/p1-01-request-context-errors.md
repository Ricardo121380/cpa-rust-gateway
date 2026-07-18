# P1-01 Request context and errors report

| Field | Value |
|---|---|
| Plan | `v1.0` |
| Task | `P1-01` |
| Date | `2026-07-18` |
| Branch | `codex/p1-01-request-context-errors` |
| Rust | `1.97.1` |
| Result | PASS |

## Delivered scope

- Added dependency-free opaque stable IDs in `gateway-core` for request, attempt, client key,
  access group, authentication, credential, provider, upstream, endpoint, public model, route,
  and route candidate identity.
- Added immutable request-level `RequestContext` carrying only `RequestId`; attempt and selected
  upstream state remain separate because a request can make multiple attempts.
- Added all 16 frozen `GatewayErrorCode` categories and an explicit `ErrorScope` that separates
  request, credential, account, model, quota window, egress session, egress, provider, stream,
  and internal remediation owners.
- Added `GatewayError` as a transport-neutral, secret-safe type. It accepts no caller diagnostic
  text; its diagnostic is fixed from the stable code.
- Added [BC-CORE-001](../contracts/BC-CORE-001-request-context-and-errors.md) and a deterministic
  error-code snapshot.

## Verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-core` | PASS; 7 unit tests plus doc tests |
| `cargo clippy --locked -p gateway-core --all-targets --all-features -- -D warnings` | PASS |
| `./scripts/check.sh fast` | PASS |
| `./scripts/check.sh full` | PASS; advisories, bans, licenses, sources, and RustSec audit pass |
| `git diff --check` | PASS |

## Review

- Requirement and dependency-boundary review confirmed that P1-01 remains entirely in
  `gateway-core` with no new dependencies.
- Independent code review found missing `QuotaWindow`/`EgressSession` remediation scopes and an
  unsafe caller-supplied diagnostic field. Both were corrected before final validation.
- Final independent review: PASS; no P1-02 CanonicalRequest, event, HTTP, provider, storage, or
  routing work was introduced.

## Limits and next task

P1-01 does not implement CanonicalRequest, CanonicalEvent, retry/failover, HTTP encoding, provider
execution, auth, or persistence. `P1-02` remains `PENDING` and is the next task only after it is
explicitly marked `IN_PROGRESS`.
