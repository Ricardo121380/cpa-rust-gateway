# G3 phase gate report

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Gate | `G3` |
| Date | `2026-07-21` |
| Verification branch | `codex/p3-10-real-endpoint-validation` |
| Tested implementation commit | `2df79fc433619794962f243bc149ec24103f5149` |
| Local result | `PASS` |
| Final status | `PASS` (GitHub Fast and Full supply-chain gates passed) |

## Conclusion

P3-01 through P3-10 are `DONE`, and all G3 exit conditions have bounded code, deterministic test,
or explicitly authorized real-test evidence. The final P3-10 validation proves the test-only
`p3-chatgpt-compat` path against two independently configured ChatGPT-family Candidates in both
non-streaming and SSE modes without retaining an Endpoint, credential, upstream model, request,
response, or raw SSE frame in tracked material.

The only transport adjustment accepted for this final validation is `CR-P3-G3-003`: the ignored
live-test profile's SSE response-idle limit is `45s`, equal to its existing finite total limit.
Connect remains `5s`, first byte remains `15s`, route bootstrap remains `45s`, and
`max_attempts=1` remains unchanged. Production transport, proxy/TUN settings, retry behavior,
Endpoint mappings, and decoder scope remain unchanged.

P4 remains `PENDING`; this Gate neither starts P4 nor creates a deployment, production Provider,
or server-side configuration change.

## G3 conditions and evidence

| Condition | Evidence | Result |
|---|---|---|
| The test-only public alias reaches two independent Candidates through the project Base URL/Key boundary | [P3-10 report](p3-10-real-test-endpoint-validation.md), [ADR-0020](../adr/ADR-0020-authorized-real-test-endpoint-validation.md), and [BC-E2E-002](../contracts/BC-E2E-002-authorized-real-test-endpoint-validation.md): authorized A/B non-streaming and SSE paths all passed with redacted evidence | PASS |
| Equal and weighted Route scheduling remains within its fixed deterministic plans | P3-03 proves exact `1:1` and repeating `5:1:1` plans, including concurrent `400:80:80` results over 560 selections. The immutable cycle derives the plan's 1000-selection check as `500:500` for equal weight and `714:143:143` for `5:1:1`, both within the required deviation; P3-04 adds the concurrent two-layer `3:1` Route / `1:1` Credential proof | PASS |
| Adding Credentials inside one Endpoint does not change Route-level target proportion | P3-04 independent Candidate and Credential cursors, with concurrent two-layer distribution assertions | PASS |
| Retry is allowed only before the first semantic event | P3-06 request-scoped Attempt orchestrator tests cover pre-semantic fallback, post-semantic closure, cancellation, and no transparent replay | PASS |
| Connection, `429`, `5xx`, and Credential saturation have distinct bounded handling | P3-02 finite transport outcomes, P3-04 lease saturation, P3-05 health shards, and P3-06 failure classification/exclusion tests | PASS |
| Request hot path has no SQLite, global scheduler mutex, or unbounded event channel | P3-02 through P3-08 crate-boundary checks and reviews; P3-08 uses bounded priority queues and P3 route/credential selection uses immutable snapshots plus atomic cursors | PASS |

## Verification record

The implementation commit's local verification includes `./scripts/check.sh full` after
`CR-P3-G3-003`, with format, Clippy, workspace tests, documentation links, secret scan, dependency
policy, and RustSec audit all passing. This Gate independently reran:

```text
cargo test --locked -p gateway-http-actix --test p3_10_real_endpoint_validation
```

All 12 non-live tests passed and the authorized real-target test remained ignored. The timeout-shape
test proves `connect=5s`, `TTFB=15s`, `idle=45s`, and `total=45s` before any network activity.
The separately authorized full-path A/B/A/B run is recorded in the P3-10 report and completed with
all four probes passing; B SSE passed as its fourth fixed probe.

GitHub Actions [29761237706](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29761237706)
passed for commit `2df79fc`:

| Job | Result |
|---|---|
| Fast gate | PASS |
| Full supply-chain gate | PASS |

One prior Full job on the same commit was cancelled after its GitHub-hosted runner remained stale
beyond the workflow's configured 45-minute job limit. It showed no code failure and is not used as
acceptance evidence; the replacement run above completed successfully on the identical SHA.

## Review

Review passed. The final diff adds named live-test timeout constants and a non-live timeout-shape
test; it does not modify any production crate. `git diff --check` passed, and the tracked change
contains only the ignored harness and documentation. The retained evidence is limited to opaque
target labels, protocol mode, and pass/fail status. No real-test configuration or raw network
material is tracked.

The live harness intentionally owns a fixed A/B non-streaming/SSE sequence. It cannot isolate B SSE
without changing that previously accepted four-probe contract, so the authorized revalidation ran
the existing fixed sequence; B SSE was reached and passed without retry or failover.

## Follow-up boundary

P3 is complete. `P4-01` remains `PENDING` and requires a new explicit start under the development
plan. This Gate does not authorize work on P4 or later phases.
