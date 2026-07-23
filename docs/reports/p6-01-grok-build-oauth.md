# P6-01 Grok Build OAuth credential and Device Code report

| Field | Value |
|---|---|
| Plan version | `v1.9` |
| Task | `P6-01` |
| Date | `2026-07-22` |
| Branch | `codex/p6-grok-build` |
| Status | `DONE` |
| Scope / budget | `M`; local OAuth credential import/Device Code state only; no real Provider traffic or server change |
| Execution channel | Current default model, `medium`; this task needs state-machine and secret-boundary review. Luna is unavailable in this execution surface; no subagent was used. |
| References | Matrix `C02`、`C28`、`E06-E10`; [ADR-0042](../adr/ADR-0042-grok-build-oauth-credential-boundary.md); [BC-CRED-003](../contracts/BC-CRED-003-grok-build-oauth-device-code.md) |

## Delivered evidence

P6-01 replaces `provider-grok`'s marker-only implementation with a local, mockable OAuth boundary.
It imports only strict bounded JSON credentials, computes expiry exclusively from `expires_in`,
redacts/zeroizes token material, discards `id_token`, and rejects duplicate fields including nested
duplicates. It does not infer an account from JWT data.

The Device Authorization state machine uses the documented fixed Device Code and token endpoints,
public client id, and scope. It gives a mock transport only safe endpoint/kind metadata; its private
payload retains form secrets. The poller prevents early calls, follows `authorization_pending`,
increases cadence on `slow_down`, and ends irreversibly after grant/deny/expiry. A direct refresh
exchange is intentionally pure; P6-02 will add singleflight, revision/CAS, and persistence.

All values in the integration test are synthetic. No process environment, database, server,
credential file, socket, proxy, or real Provider endpoint is used.

## Verification

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p6_01_build_oauth` | PASS; 4 synthetic import, Device Code, and refresh-boundary tests passed. |
| `cargo clippy --locked -p provider-grok --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` and `git diff --check` | PASS |
| `ruby scripts/check-crate-boundaries.rb` | PASS; 21 workspace packages and the narrow `serde`/`serde_json`/`url`/`zeroize` Provider edge accepted. |
| `./scripts/check.sh full` | PASS; shell/CI/plan guards, workspace format/Clippy/tests, source policy, docs links, staged tracked-Secret scan, whitespace, dependency policy, RustSec audit, and supply-chain checks passed. |
| Staged Secret scan and independent code review | PASS; staged scan found no credential pattern. Review corrected Device Code client/scope inheritance so an omitted token-response field cannot downgrade a custom flow to the default public client. |

## Review conclusion

The review separates OAuth parsing from account identity and network access. A duplicate-key parser
is necessary because ordinary JSON map decoding would otherwise overwrite an earlier token field.
The public transport request is a private-payload struct rather than a public enum so test/mocking
code can verify endpoint/kind without gaining a pattern-match path to Device or refresh secrets.

The expiration rule deliberately prefers an explicit failure to an epoch-unit guess. The parser also
does not retain OAuth `error_description`, raw response JSON, or `id_token`; resulting error values
are static classifications. This leaves P6-02 through P6-07 clear ownership boundaries instead of
embedding refresh, persistent state, Build HTTP, quota, cache, or provider-error policy into P6-01.

## Rollback and next task

Reverting P6-01 removes only `provider-grok` OAuth code, synthetic tests, documents, and the direct
locked dependency edges. It requires no migration, credential revocation, service restart, server
cleanup, or real Provider action. After the completed local full gate, P6-01 remains
`LOCAL_PASS_PENDING_PHASE_GATE`; P6-02 becomes the sole `IN_PROGRESS` Task and owns refresh
singleflight, revision/CAS, and persistence. G6 remains the Phase's only remote Delivery Gate.
