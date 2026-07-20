# P3-10 real-test Endpoint validation plan

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P3-10` |
| Matrix / behavior | `C16`, `G05`, `G12-G15`, `G21`, `K03-K06`, `L20-L31`; Behavior 1/4/5/9/17/20 |
| Date | `2026-07-20` |
| Branch | `codex/p3-10-real-endpoint-validation` |
| Status | IN_PROGRESS — three attempts total have stopped at Target `A` / non-streaming; the first `p3-chatgpt-compat` attempt safely classified as `5xx`, and no further revised-boundary traffic may occur without a new user-directed plan |

## Entry review

P3-09's final GitHub CI run
[29713118335](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29713118335) completed
successfully for commit `a9fb25d`. Therefore P3-10 is the plan's only `IN_PROGRESS` Task.

The repository contains no tracked real-test Base URL, credential, model, proxy profile, or request
budget. No ambient API-key variable, prior chat text, local shell history, or test-only P3-09 value
is eligible to fill those fields. An authorized operator-controlled file under ignored
`docs/reports/private/` is the only permitted local source for a real run.

## Approved Change Request: CR-P3-G3-001

```text
CR-ID: CR-P3-G3-001
原因: P3-10/G3 曾用 minimax-m3 作为真实测试的公开模型名，但两个 operator-controlled
      私有 Candidate 映射属于 ChatGPT-family 上游；该命名会把客户端兼容性别名误读为
      上游模型身份。
影响的 Task / Matrix ID / ADR: Plan P3-10、G3、BC-E2E-002、ADR-0020 与
      p3_10_real_endpoint_validation；不修改 P3-09 的 Mock fixture，也不决定 P10 以后的
      产品 PublicModel。
兼容性与迁移影响: 无 API、Schema、数据库或部署迁移。P3-10 的 client-visible test-only
      public model 变为 p3-chatgpt-compat，输入别名为 p3-chatgpt-compat-alias；A/B 仍各自
      使用私有配置中明确的上游模型，且不会写入 Git 或输出。
测试与回滚变化: 非实时测试继续验证请求别名、公开模型回写、脱敏和 output cap；批准后重跑
      完整门禁。回滚仅能通过新的已批准 CR 恢复原文字/常量；不产生外部流量。
用户批准: APPROVED，2026-07-20
授权边界: 先前未使用的真实调用额度不自动覆盖这个变更后的公开测试边界；新的真实运行必须
      有单独的调用数与预算授权。
```

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
- Its client-visible model is the test-only `p3-chatgpt-compat` alias; each Candidate's actual
  ChatGPT-family upstream mapping remains private and is not an identity claim in this report.

## Local verification evidence

| Command / review | Result |
|---|---|
| `cargo test --locked -p gateway-http-actix --test p3_10_real_endpoint_validation` | PASS; eight non-live guard/config/privacy/output-cap checks passed and the real target remained ignored. |
| Ignored private mapping family check | PASS; both nonempty A/B upstream-model mappings matched the authorized ChatGPT-family shape without rendering either value or constructing transport. This is selection evidence only, not a relay capability claim. |
| Ignored target with every `P3_10_*` variable unset | EXPECTED SAFE STOP; `NotAuthorized` returned in about 0.01 seconds with exit status 101 before Endpoint/harness construction, DNS resolution, or transport. This is guard evidence, not a real-target result. |
| `cargo test --locked -p gateway-http-actix --test p3_09_aggregation_e2e` | PASS; 3/3 controlled Mock E2E regressions still use the shared test-only harness. |
| `cargo test --locked -p gateway-router credential_scheduler::tests::two_layer_atomic_cursors_preserve_route_and_endpoint_weights_under_concurrency -- --exact` (50 repetitions) | PASS; cursor-distribution assertion is now isolated from temporary lease saturation. |
| `cargo clippy --locked -p gateway-router -p gateway-http-actix --all-targets --all-features -- -D warnings` and `cargo fmt --all -- --check` | PASS. |
| `./scripts/check.sh full` | PASS after `CR-P3-G3-001`; complete workspace format, Clippy, tests, source policy, crate boundaries, document links, tracked-secret scan, dependency policy, and RustSec audit passed. The ignored live target remained unexecuted. |

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
P3 as complete: successful real-target compatibility evidence for both Candidates and both modes
is still absent. The two historical attempts recorded below stopped at Target `A` / non-streaming
under the superseded public alias, and the first revised `p3-chatgpt-compat` attempt also stopped
at Target `A` / non-streaming. They establish neither Target `B` nor either SSE path.

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

### 2026-07-20 follow-up authorization record

The user subsequently authorized up to four additional real probes (five total including the
previous request), with a total spend ceiling of USD 10. The fixed `2xx`-accepting harness was run
once under that authorization. It stopped at its first probe as required.

| Field | Recorded safe result |
|---|---|
| Second probe | Target `A`, non-streaming |
| Result | STOPPED after one request because the gateway response was not `2xx` |
| Retained status | `5xx` class only; no exact status, URL, body, model, credential, or provider diagnostic was retained |
| Further traffic | None: Target `B`, both SSE probes, retry, and failover were not invoked |
| Authorization usage | One of the four newly authorized probes was sent; the remaining three were deliberately left unused after the anomaly. No cost figure was collected or retained. |
| Consequence | P3-10 and G3 remain unaccepted. The unused call capacity is not a basis for an automatic retry, resume, provider switch, proxy/TUN change, or decoder change. |

This is the terminal record for that follow-up run: it demonstrates that the revised harness classifies a
non-success response safely, but it provides no successful real-endpoint compatibility evidence.

The user later approved `CR-P3-G3-001`. These two historical attempts remain a safe record only;
they do not authorize or satisfy a real run under the revised public alias.

### 2026-07-20 revised-boundary authorization record

The user explicitly authorized a fresh P3-10 run under the revised `p3-chatgpt-compat` boundary:
up to four new probes with a USD 10 maximum budget. A no-transport preflight confirmed the fixed
four-call cap, direct profile, and two distinct redacted ChatGPT-family mappings before the ignored
target was enabled. The run stopped at its first probe as required.

| Field | Recorded safe result |
|---|---|
| Third probe / first revised-boundary probe | Target `A`, non-streaming |
| Result | STOPPED after one request because the gateway response was not `2xx` |
| Retained status | `5xx` class only; no exact status, URL, body, model, credential, or provider diagnostic was retained |
| Further traffic | None: Target `B`, both SSE probes, retry, and failover were not invoked |
| Authorization usage | One of the four revised-boundary probes was sent; the remaining three were deliberately left unused after the anomaly. No cost figure was collected or retained. |
| Consequence | P3-10 and G3 remain unaccepted. The matching safe `5xx` classification under both the superseded and revised public aliases is not sufficient to attribute a cause; it is not a basis for an automatic retry, provider switch, proxy/TUN change, or decoder change. |

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

- Each approved target has one successful bounded non-streaming and SSE compatibility result through
  test-only `p3-chatgpt-compat`, or a clearly redacted stopped finding with no further unapproved
  calls. A stopped finding is safe execution evidence, not P3-10 completion evidence.
- The project boundary keeps that test-only public alias client-visible and does not expose upstream
  model, Endpoint, credential, or raw response data.
- Request/Attempt/Usage correlation, timeout/proxy selection, and first-semantic-event semantics
  are evidenced without retaining their secret-bearing values.
- The ignored target, documentation, source policy, secret scan, Fast gate, Full gate, and three
  GitHub CI acceptance records all pass before P3-10 is marked `DONE`.

## Scope boundary

P3-10 does not deploy a server, mutate Clash/TUN rules, create a generic production decoder,
discover models, write SQLite events, rotate credentials, or perform P4/P5 work. Its purpose is the
smallest authorized real-target validation of the already-composed P3 aggregation path.
