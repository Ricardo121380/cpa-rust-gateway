# P13-06C report: Provider account operator actions and failure feedback

Status: `DONE_WITH_BOUNDARY`

## Objective

Extend the P13-06B read-only provider account-pool projection with narrowly bounded operator
actions and a safe failure-feedback read model. Preserve the existing serving pool and runtime
registries as the only source of truth.

## Frozen implementation boundary

- exact Provider/Channel/Account targeting only;
- local Health cooldown and controlled Health/Quota recovery only;
- Management Key, same-origin CSRF, selected Config Version and resource audit;
- no Provider/network request, lease acquisition, Config Version publication, credential rewrite,
  automatic refresh/reauth, or proxy-pool work;
- durable Attempt failure classification only, with no raw upstream values;
- OpenAPI/Prism contract handoff is logged for Claude Code; no formal Prism UI in this slice.

## Evidence ledger

| Area | Evidence | Status |
|---|---|---|
| Scope / contract | ADR-0083, BC-MGMT-016, plan v1.253 | `PASS` |
| Operator action state machine | gateway package (99 tests), including exact model cooldown, stale/unknown/disabled target rejection and quota recovery state-machine regressions | `PASS` |
| Failure feedback projection | gateway-control package (57 tests) plus protected HTTP inventory fixture; newest-first filtering/cursor and no-model/no-raw-value assertions | `PASS` |
| Authentication / CSRF / audit | protected management HTTP tests (gateway-http-actix `--tests`: 118 passed, 4 ignored); action path uses existing Management Key, same-origin CSRF, Config Version admission and resource audit | `PASS` |
| OpenAPI / Prism handoff | OpenAPI contract tests 9/9; `npm --prefix web/prism run sync-contract`, `check`, `type-check`, `build`; cross-boundary log updated | `PASS_WITH_CLAUDE_UI_HANDOFF` |
| Provider / production | deliberately not called or changed | `NOT_IN_SCOPE` |

## Review verdict

`PASS_WITH_BOUNDARY`: the backend slice is complete and review-clean after the unified P13 phase
Delivery Gate run
[31858904767](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31858904767). The adapter mutates only
the already shared in-process Health/Quota registries; it does not send Provider requests, acquire
leases, refresh or reauthenticate credentials, publish a Config Version, or change production
traffic. Failure feedback is compiled only from typed durable `AttemptEvent` classifications and
is bounded to opaque attribution, timestamps, closed error code/scope and retry decision.

The management OpenAPI source and Prism generated contract/client are synchronized. Claude Code
must still integrate the generated `applyProviderAccountPoolAction` and
`listProviderAccountFailures` methods into Prism state/UI with explicit confirmation, safe error
display, pagination and stale-target (`409`) handling. No Prism application code was hand-edited
in this backend slice.

## Verification ledger

- `cargo test --locked -p gateway --all-targets`: 99 passed.
- `cargo test --locked -p gateway-control --all-targets`: 57 passed.
- `cargo test --locked -p gateway-http-actix --tests -- --test-threads=1`: 118 passed, 4 ignored
  (the ignored tests are separately authorized/live or soak-only tests).
- P13-06C OpenAPI/HTTP focused tests: contract 9/9 and management inventory 1/1.
- `cargo test --locked -p provider-grok --test p12_10b_native_account_pool --test
  p12_10c_native_account_scheduling --test p12_10d_native_account_workers`: 21 passed.
- Strict Clippy for touched runtime/control/HTTP/router/upstream packages: passed with `-D warnings`.
- Prism `check`, `type-check`, and `build`: passed; generated client was not hand-edited.
- `cargo fmt --all -- --check`, `git diff --check`, OpenAPI JSON validation, docs/link/contract/plan/
  secret checks: passed.
- Unified local phase preflight: [`p13-phase-preflight-20260815.md`](evidence/p13-phase-preflight-20260815.md)
  records the authoritative Full gate (`43/43` steps PASS), Prism Vitest (`157/157` PASS) and the
  formal remote Delivery Gate (`31858904767` all required jobs PASS).

The full `--all-targets` HTTP command was also checked; one run exposed an unrelated temporary-file
collision in the pre-existing backup test and a `--test-threads` argument being forwarded to an
existing benchmark. The affected backup test passed when rerun serially, and the authoritative
`--tests -- --test-threads=1` suite passed in full; no P13 failure remains.
