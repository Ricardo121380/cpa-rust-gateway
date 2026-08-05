# P12-10I-01 Native Grok Reauthentication — Local Receipt

Date: 2026-08-05
Scope: local CPAR implementation only
Change request: `CR-P12-10I-001`

## Outcome

The useful control-flow behavior inspected in the Jakarta `grok-register` project is now available
as a CPAR-native, provider-independent reauthentication boundary:

- refresh-first fallback is ordered as `Refresh -> DeviceCode -> BrowserSso` for `Auto`;
- `RefreshOnly` and `InteractiveOnly` make the allowed action set explicit;
- each pass claims at most 200 accounts and executes them serially (fixed concurrency of one);
- a replacement is validated against the account's provider shape, sealed with the existing
  account-pool AEAD boundary, and committed only at the expected revision and live claim;
- transient failures use bounded deterministic backoff;
- denied or operator-required outcomes are durably blocked until an explicit `requeue_reauth` handoff;
- Device Code and external browser/SSO operations are injected in memory, so CPAR does not read a
  password store, open a browser, persist a password/SSO/Bearer value, or call a source project's
  upstream directly.

This is a local implementation receipt, not a live OAuth result and not a production deployment.

## Source behavior mapping

The read-only source review covered the Jakarta project's refresh grant, interactive fallback,
serial worker, bounded batch, and per-account result handling. CPAR retains those semantics while
replacing the old CPA sink with the native encrypted account pool and revision/CAS state. The
source project's same-email/grok2api helper is deliberately not a CPAR runtime dependency.

| Source behavior | CPAR boundary |
|---|---|
| Refresh-first retry order | `GrokReauthStrategy::Auto` |
| One account at a time | `GrokReauthCoordinator` fixed serial loop |
| Bounded batch | `MAX_GROK_REAUTH_BATCH = 200` |
| Per-account durable outcome | `grok_account_reauth_state` |
| Immediate credential replacement | `complete_reauth_success` with revision/claim CAS |
| Browser/SSO handoff | `GrokReauthExecutor` + `requeue_reauth` |

## Local evidence

| Check | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p12_10i_grok_reauth` | PASS: 3/3 |
| `cargo test --locked -p gateway-store` | PASS: 41/41 (37 unit, 1 repository, 3 backup/upgrade) |
| `cargo clippy --locked -p provider-grok --tests --all-features -- -D warnings` | PASS |
| `CHECK_REPORT_PATH=... ./scripts/check.sh fast` | PASS; the command covered the full local gate and retained a value-free ignored log in the working tree |
| schema migration/rollback coverage | PASS in `gateway-store` test suite |
| serial invariant | PASS: test executor observed maximum in-flight = 1 |
| restart/lease invariant | PASS: unexpired claim is not duplicated; expired claim is reclaimable |
| manual interaction invariant | PASS: `NeedsInteractive` is blocked until explicit requeue |
| secret diagnostics | PASS: job/result debug output redacts credential material; receipt contains no credential values |

## Boundary and rollback

No network request, browser window, OAuth exchange, server process, deployment graph, Caddy/DNS
configuration, grok2api state, or production account was touched. The only persistence change is
schema migration `0013_native_grok_reauth`; its down migration drops the new table and index. The
implementation is not wired into a running deployment until a later task explicitly supplies an
executor and its separate authorization boundary.
