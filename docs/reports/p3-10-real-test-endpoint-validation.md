# P3-10 real-test Endpoint validation plan

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P3-10` |
| Matrix / behavior | `C16`, `G05`, `G12-G15`, `G21`, `K03-K06`, `L20-L31`; Behavior 1/4/5/9/17/20 |
| Date | `2026-07-20` |
| Branch | `codex/p3-10-real-endpoint-validation` |
| Status | IN_PROGRESS — first authorized probe stopped at the prior exact-`200` check; a fixed `2xx`-accepting harness awaits explicit clean-rerun authorization |

## Entry review

P3-09's final GitHub CI run
[29713118335](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29713118335) completed
successfully for commit `a9fb25d`. Therefore P3-10 is the plan's only `IN_PROGRESS` Task.

The repository contains no tracked real-test Base URL, credential, model, proxy profile, or request
budget. No ambient API-key variable, prior chat text, local shell history, or test-only P3-09 value
is eligible to fill those fields. An authorized operator-controlled file under ignored
`docs/reports/private/` is the only permitted local source for a real run.

## Delivered local harness

- Added ignored target `p3_10_real_endpoint_validation` plus the shared test-only
  `tests/support/p3_aggregation.rs` composition harness. P3-09 now uses the same Router, admitted
  transport, bounded decoder, Snapshot HTTP boundary, and event path; no production library target
  gained a concrete Provider dependency.
- Ordinary tests execute six synthetic guard/config/privacy checks. The real test is `#[ignore]` and
  first requires `P3_10_LIVE_AUTHORIZATION=approved`, all A/B-specific variables, and exactly
  `P3_10_MAX_EXTERNAL_REQUESTS=4`; it never reads `.env`, `OPENAI_API_KEY`, `CODEX_API_KEY`, or
  any other ambient provider variable.
- The live route has two Candidates, but `max_attempts=1`. It sends one non-streaming and one SSE
  probe to each Candidate in deterministic A/B/A/B order, each with the fixed non-sensitive input
  and `max_output_tokens=32`, so a failed target stops immediately instead of retrying, failing
  over, or exceeding the four approved calls.
- The P3-10 harness allocates a distinct opaque Request ID for each of those four probes and checks
  Request/Attempt/Usage equality per probe without rendering any correlation value. P3-09 retains
  its fixed deterministic fixture ID.
- Upstream JSON reads are capped at 64 KiB, each SSE frame at 16 KiB, and client-body verification
  at 64 KiB under a finite transport deadline. The retained console summary contains only target
  label, mode, and pass/fail; runtime checks reject a client-visible Base URL, upstream model,
  upstream credential, or project Client Key.
- A response is successful only when its HTTP status is `2xx`, not merely `200`. A stopped run
  prints only its opaque target label, mode, and safe status class; it never prints an exact status,
  URL, model, credential, body, or provider diagnostic.

## Local verification evidence

| Command / review | Result |
|---|---|
| `cargo test --locked -p gateway-http-actix --test p3_10_real_endpoint_validation` | PASS; seven non-live guard/config/privacy/output-cap checks passed and the real target remained ignored. |
| Ignored target with every `P3_10_*` variable unset | EXPECTED SAFE STOP; `NotAuthorized` returned in about 0.01 seconds with exit status 101 before Endpoint/harness construction, DNS resolution, or transport. This is guard evidence, not a real-target result. |
| `cargo test --locked -p gateway-http-actix --test p3_09_aggregation_e2e` | PASS; 3/3 controlled Mock E2E regressions still use the shared test-only harness. |
| `cargo test --locked -p gateway-router credential_scheduler::tests::two_layer_atomic_cursors_preserve_route_and_endpoint_weights_under_concurrency -- --exact` (50 repetitions) | PASS; cursor-distribution assertion is now isolated from temporary lease saturation. |
| `cargo clippy --locked -p gateway-router -p gateway-http-actix --all-targets --all-features -- -D warnings` and `cargo fmt --all -- --check` | PASS. |
| `./scripts/check.sh full` | PASS; complete workspace format, Clippy, tests, source policy, crate boundaries, document links, tracked-secret scan, dependency policy, and RustSec audit passed. |

## Review

Implementation review passed. The live test reaches no configuration or URL parsing before its
exact authorization check, and its ignored status prevents ordinary local/CI checks from invoking
it. Once explicitly enabled, the fixed four-request loop owns one non-streaming and one SSE probe
per Candidate, with `max_attempts=1`; a failure returns from the loop before another probe, retry,
or failover can occur. The response and event assertions operate only on bounded material and do
not print correlation values or upstream-private inputs.

The probe now explicitly carries `max_output_tokens=32`. The local test decodes the fixed public
payload and rebuilds the concrete OpenAI-compatible outbound body, proving the cap survives the
Canonical extension boundary for both non-streaming and SSE modes before any real request is made.

The first complete workspace gate exposed an existing P3-04 test instability: its strict
cursor-fairness assertion also allowed the test pools to saturate transiently under eight workers.
That is a different availability behavior, already covered by saturation tests, so the regression
fixture now gives every test pool capacity for all active workers. No production scheduling,
Credential, transport, or P3-10 behavior changed; 50 repeated executions prove the intended
cursor-only assertion is deterministic.

This review accepts only the local harness and its safe guard. It does not accept P3-10, G3, or
P3 as complete: the four authorized real-target calls and their redacted outcome remain required.

## Authorized real-run record

The user authorized use of the local CCSwitch `krill` and `帅api` Codex configurations. The harness
generated a `0600`, Git-ignored local configuration from those two distinct HTTPS Responses sources;
no tracked file received an Endpoint, model, or credential.

| Field | Recorded safe result |
|---|---|
| First probe | Target `A`, non-streaming |
| Result | STOPPED after one request because the then-current harness rejected a status other than exact `200` |
| Retained status | Not available: that revision did not retain a status class, URL, body, or provider diagnostic |
| Further traffic | None: Target `B`, both SSE probes, retry, and failover were not invoked |
| Consequence | P3-10 remains unresolved; the harness now accepts any `2xx` and retains only a safe class for a future explicitly authorized clean rerun |

The one request counts against the original four-call authorization. A clean rerun must receive a
new explicit authorization and spend cap before it can send any additional traffic.

## GitHub CI

GitHub Actions run [29716094066](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29716094066)
passed for harness implementation commit `691159f`.

| Job | Result |
|---|---|
| Fast gate | PASS; completed `2026-07-20T04:09:06Z` |
| Full supply-chain gate | PASS; completed `2026-07-20T04:20:14Z` |

This remote result accepts the opt-in local harness, its test-boundary refactor, and the
cursor-fairness fixture isolation. It does not authorize a real Endpoint or alter the task's
`IN_PROGRESS` status. The verification-record commit below must pass the same workflow before this
local evidence is considered durable.

## Execution protocol

1. Review the configuration below and source only the ignored private file into the current shell.
2. Explicitly authorize the four fixed external calls and a maximum spend before running the exact
   ignored target command below.
3. On the first non-2xx, 429, 5xx, timeout, protocol mismatch, missing Usage event, or redaction
   violation, stop. Do not modify a proxy/TUN rule, retry, or broaden a decoder during the run.
4. Record only the target label, mode, safe outcome category, and boolean invariants in this report;
   any raw material remains ignored under `docs/reports/private/` and is deleted after review.

## Required operator inputs and authorization

Supply these through an ignored local file and source it into the terminal; do not paste secrets
into chat or a tracked file. A suggested local file is `docs/reports/private/p3-10.env`:

```bash
export P3_10_LIVE_AUTHORIZATION=approved
export P3_10_MAX_EXTERNAL_REQUESTS=4
export P3_10_ENDPOINT_A_BASE_URL='https://test-relay-a.example/v1'
export P3_10_ENDPOINT_A_API_KEY='test-only-key-a'
export P3_10_ENDPOINT_A_UPSTREAM_MODEL='provider-model-a'
export P3_10_ENDPOINT_B_BASE_URL='https://test-relay-b.example/v1'
export P3_10_ENDPOINT_B_API_KEY='test-only-key-b'
export P3_10_ENDPOINT_B_UPSTREAM_MODEL='provider-model-b'
export P3_10_NETWORK_PROFILE=direct
```

For an already configured local-DNS SOCKS5 profile, replace the final line with the following. The
proxy URL must be credential-free `socks5://host:port`; HTTP, HTTPS, and `socks5h` are rejected.

```bash
export P3_10_NETWORK_PROFILE=socks5
export P3_10_SOCKS5_PROXY_URL='socks5://127.0.0.1:7891'
```

If and only if an authorized test relay resolves to a private/local address, add one explicit
narrow CIDR for that target (for example `127.0.0.1/32`). Public targets need no CIDR override:

```bash
export P3_10_ENDPOINT_A_ALLOWED_CIDR='127.0.0.1/32'
```

Before execution, the operator must also explicitly confirm:

- both targets are authorized test relays using the OpenAI-compatible Responses endpoint;
- the permitted request count and maximum spend, including any two-candidate confirmation calls;
- whether traffic must use `direct` or a preconfigured SOCKS5 profile; and
- that a `401`, `403`, quota/billing response, transport failure, or unexpected response stops
  rather than triggers an automatic retry.

The placeholders above are documentation only. They are not usable configuration, and the harness
will not contact them.

After sourcing the private file, the only live command is:

```bash
cargo test --locked -p gateway-http-actix --test p3_10_real_endpoint_validation -- --ignored --nocapture
```

## Acceptance criteria

- Each approved target has one successful bounded non-streaming and SSE compatibility result, or a
  clearly redacted stopped finding with no further unapproved calls.
- The project boundary keeps the stable public model client-visible and does not expose upstream
  model, Endpoint, credential, or raw response data.
- Request/Attempt/Usage correlation, timeout/proxy selection, and first-semantic-event semantics
  are evidenced without retaining their secret-bearing values.
- The ignored target, documentation, source policy, secret scan, Fast gate, Full gate, and three
  GitHub CI acceptance records all pass before P3-10 is marked `DONE`.

## Scope boundary

P3-10 does not deploy a server, mutate Clash/TUN rules, create a generic production decoder,
discover models, write SQLite events, rotate credentials, or perform P4/P5 work. Its purpose is the
smallest authorized real-target validation of the already-composed P3 aggregation path.
