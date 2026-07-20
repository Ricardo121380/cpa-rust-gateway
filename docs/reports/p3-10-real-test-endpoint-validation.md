# P3-10 real-test Endpoint validation plan

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P3-10` |
| Matrix / behavior | `C16`, `G05`, `G12-G15`, `G21`, `K03-K06`, `L20-L31`; Behavior 1/4/5/9/17/20 |
| Date | `2026-07-20` |
| Branch | `codex/p3-10-real-endpoint-validation` |
| Status | LOCAL PASS — all four authorized full-path probes pass after `CR-P3-G3-003`; final P3/G3 verification record and GitHub acceptance remain pending |

## Entry review

P3-09's final GitHub CI run
[29713118335](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29713118335) completed
successfully for commit `a9fb25d`. At that entry point, P3-10 became the plan's only
`IN_PROGRESS` Task.

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

## Approved Change Request: CR-P3-G3-002

```text
CR-ID: CR-P3-G3-002
原因: 已授权 Target A 的有效 SSE 生命周期帧超过原先 test-only 16 KiB 限制；有界结构诊断
      已确认事件序列完整且未保留原始帧。
影响的 Task / Matrix ID / ADR: P3-10、G3、BC-E2E-002、ADR-0020 与共享 test-only
      aggregation harness；不修改 P3-09 的一秒 Mock deadline、生产 Provider、API、Schema、
      数据库、部署或代理/TUN 配置。
兼容性与迁移影响: 单帧上限提升到仍有限的 64 KiB，并与既有上游 JSON / 客户端响应有界读
      对齐。大于 64 KiB 的帧继续安全拒绝且不追加到缓存；无迁移。
测试与回滚变化: 增加正好达到上限可接受、超过上限返回安全错误并保持缓存长度不变的测试；
      回滚只能经新的已批准 CR 恢复旧限制。
用户批准: APPROVED，2026-07-20
```

## Approved Change Request: CR-P3-G3-003

```text
CR-ID: CR-P3-G3-003
原因: Target B 的完整 P3 SSE 探测以安全 `EgressUnavailable` 停止；直接的有界 SSE
      诊断已证明该 relay 可以完成等价请求。现有 public failure 分类无法区分 15 秒首字节
      与 20 秒响应 idle 边界。
影响的 Task / Matrix ID / ADR: P3-10、G3、BC-E2E-002、ADR-0020 与
      p3_10_real_endpoint_validation；仅修改 ignored live-test profile。
兼容性与迁移影响: SSE response-idle 从 20 秒提高至现有 45 秒总上限；connect=5 秒、
      TTFB=15 秒、Route bootstrap=45 秒、max_attempts=1 保持不变。该 test-only
      profile 的已归还连接保留期也相应最多为 45 秒；无生产 API、Schema、数据库、部署、
      proxy/TUN 或自动重试变化。
测试与回滚变化: 增加 timeout 形状断言；批准后重跑完整真实 A/B/A/B 聚合路径直到 B SSE。
      回滚只能经新的已批准 CR 恢复 20 秒限制。
用户批准: APPROVED，2026-07-21
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
- Upstream JSON reads, each SSE frame, and client-body verification are capped at 64 KiB under a
  finite transport deadline. The P3-10 Route bootstrap deadline is the same 45
  seconds as its already-bounded total transport deadline; P3-09's controlled Mock calls retain
  their prior one-second deadline. The retained console summary contains only target label, mode,
  pass/fail, and (for a locally generated error envelope) one whitelisted Gateway error category.
  Runtime checks reject a client-visible Base URL, upstream model, upstream credential, or project
  Client Key.
- A response is successful only when its HTTP status is `2xx`, not merely `200`. A stopped run
  prints only its opaque target label, mode, and safe status class; it never prints an exact status,
  URL, model, credential, body, or provider diagnostic.
- Its client-visible model is the test-only `p3-chatgpt-compat` alias; each Candidate's actual
  ChatGPT-family upstream mapping remains private and is not an identity claim in this report.

## Local verification evidence

| Command / review | Result |
|---|---|
| `cargo test --locked -p gateway-http-actix --test p3_10_real_endpoint_validation` | PASS; twelve non-live guard/config/privacy/output-cap/error-redaction/frame-boundary/timeout-shape checks passed and the real target remained ignored. |
| Ignored private mapping family check | PASS; both nonempty A/B upstream-model mappings matched the authorized ChatGPT-family shape without rendering either value or constructing transport. This is selection evidence only, not a relay capability claim. |
| Ignored target with every `P3_10_*` variable unset | EXPECTED SAFE STOP; `NotAuthorized` returned in about 0.01 seconds with exit status 101 before Endpoint/harness construction, DNS resolution, or transport. This is guard evidence, not a real-target result. |
| `cargo test --locked -p gateway-http-actix --test p3_09_aggregation_e2e` | PASS; 5/5 checks: three controlled Mock E2E regressions plus the shared finite-frame boundary checks. |
| `cargo test --locked -p gateway-router credential_scheduler::tests::two_layer_atomic_cursors_preserve_route_and_endpoint_weights_under_concurrency -- --exact` (50 repetitions) | PASS; cursor-distribution assertion is now isolated from temporary lease saturation. |
| `cargo clippy --locked -p gateway-router -p gateway-http-actix --all-targets --all-features -- -D warnings` and `cargo fmt --all -- --check` | PASS. |
| `./scripts/check.sh full` | PASS after `CR-P3-G3-001`; complete workspace format, Clippy, tests, source policy, crate boundaries, document links, tracked-secret scan, dependency policy, and RustSec audit passed. The ignored live target remained unexecuted. |
| `./scripts/check.sh full` after `CR-P3-G3-002` | PASS; format, Clippy, workspace tests, source policy, crate boundaries, document links, tracked-secret scan, dependency policy, and RustSec audit all passed. |
| `./scripts/check.sh full` after `CR-P3-G3-003` | PASS; format, Clippy, workspace tests, source policy, crate boundaries, document links, tracked-secret scan, dependency policy, and RustSec audit all passed. |

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

### 2026-07-20 transport-boundary correction and current evidence

A later user direction removed the per-probe count and spend concern for focused diagnostics. The
ignored P3-10 harness itself still has its fixed A/B/A/B, one-Attempt-per-request structure; no
automatic retry, failover, proxy/TUN mutation, Endpoint substitution, or decoder broadening was
performed.

The shared harness had previously fixed its Route bootstrap deadline at one second even though
P3-10's configured direct transport already allowed a bounded 45-second total request. That outer
deadline could cancel a live Attempt before the transport produced its first semantic event and
misclassify the local result as `EgressUnavailable`. The harness now takes an explicit positive
bootstrap `Duration`: P3-09 continues to pass one second for its controlled Mock peers, while P3-10
passes the same 45-second total bound used by its transport. This is test-only boundary alignment;
it does not alter a production timeout, retry policy, decoder, proxy behavior, API, Schema, or
deployment.

The stopped-run output now parses only a locally generated Gateway error envelope and emits a code
from the frozen Gateway-code allowlist. Unknown strings collapse to
`unrecognized_or_non_gateway_error`; no body text or upstream detail is retained or printed.

| Probe / diagnostic | Safe result |
|---|---|
| Target `A`, non-streaming, through full P3 aggregation | PASS |
| Target `B`, non-streaming, through full P3 aggregation | PASS |
| Target `A`, SSE, through full P3 aggregation | STOPPED before a public response with `StreamTruncated`; Target `B` SSE was not invoked |
| Direct, bounded A SSE structural check | `2xx` event stream with the required created, assistant-message, nonempty-text, and completed categories; no malformed or unterminated frame was retained |
| Direct, bounded A SSE frame-size check | One or more individual frames exceeded the existing 16 KiB cap; the affected categories were `response.created`, `response.in_progress`, and `response.completed` |

The A SSE finding is a real compatibility failure against P3-10's existing 16 KiB single-frame
limit, not evidence that the decoder should silently accept larger frames. P3-10 remains
`IN_PROGRESS`; accepting a larger bounded frame limit or selecting a separately authorized relay
with compliant SSE frame sizes requires an explicit user-approved change request. The direct
diagnostic material was processed in memory and discarded; no Endpoint, credential, model,
provider response ID, request body, response body, or raw frame is tracked.

### 2026-07-20 alternate-A selection attempt

The user requested an alternate frame-size-compliant source for opaque Target `A`. The only other
locally configured, distinct Responses source was assigned to `A` in the invoking process, while
the former `A` source was assigned to `B`; the ignored private configuration file was not edited.
This preserves a two-source route and puts the candidate's non-streaming and SSE checks before the
previously observed frame-cap finding.

| Probe | Safe result |
|---|---|
| Alternate Target `A`, non-streaming, through full P3 aggregation | STOPPED with `5xx` and locally generated Gateway category `EgressUnavailable` |
| Alternate Target `A`, SSE | Not invoked after the non-streaming anomaly |
| Target `B`, both modes | Not invoked |

The candidate therefore has no current frame-size-compliance result: it did not reach the SSE
decoder. The earlier successful non-streaming observation for the same source does not establish
availability or authorize a retry, proxy/TUN change, or protocol relaxation. No Endpoint,
credential, upstream model, request/response body, SSE frame, or provider diagnostic was retained.
P3-10 remained `IN_PROGRESS` pending either a new stable, separately authorized Responses relay or
an explicit change request for a different finite frame bound. The latter was subsequently
approved as `CR-P3-G3-002`; its verification record follows.

### 2026-07-20 finite-frame-bound correction and current evidence

`CR-P3-G3-002` raises the shared test-only single-frame bound from 16 KiB to 64 KiB. It does not
relax the bounded decoder into an unbounded reader: data at the cap is accepted, data above it is
safely rejected before append, and no raw frame is logged, retained, or committed.

The ordinary P3-09/P3-10 integration targets now include those boundary tests. After local
verification, the original two-target P3-10 mapping was rerun through the full aggregation path:

| Probe / diagnostic | Safe result |
|---|---|
| Target `A`, non-streaming, through full P3 aggregation | PASS |
| Target `B`, non-streaming, through full P3 aggregation | PASS |
| Target `A`, SSE, through full P3 aggregation | PASS at the approved finite frame bound |
| Target `B`, SSE, through full P3 aggregation | STOPPED before a public response with `5xx` and Gateway category `EgressUnavailable` |
| Target `B`, bounded direct SSE diagnostic using gateway-equivalent request shape over HTTP/1.1 | `2xx` event stream completed without retaining response data |
| Target `B`, bounded direct SSE diagnostic using gateway-equivalent request shape over HTTP/2 | no response before the finite timeout; no response data retained |

The fourth full-path probe establishes a distinct transport compatibility finding: the B relay can
complete the same bounded SSE request in a direct HTTP/1.1 diagnostic, while a separately requested
HTTP/2 direct diagnostic did not produce a timely first byte. That contrast is not proof that the
shared gateway client negotiated HTTP/2. Its current native-TLS/ALPN configuration must be verified
before any HTTP-version policy is considered. The gateway's existing P3-10 profile separately has
finite 15-second first-byte and 20-second body-idle bounds, and both transport failures map to the
same safe `EgressUnavailable` category; the retained public result cannot distinguish them. The
finding is not a frame-cap, credential, Endpoint, proxy/TUN, or ordinary decoder semantic mismatch.
P3-10 and P3 remained `IN_PROGRESS` pending an explicit, evidence-backed test-only transport
decision. That decision was subsequently approved as `CR-P3-G3-003`; its revalidation record
follows. No automatic retry, failover, Endpoint substitution, proxy/TUN mutation, or shared-
transport change was made.

### 2026-07-21 idle-bound revalidation

`CR-P3-G3-003` raises only the ignored P3-10 live-test SSE response-idle bound from 20 seconds to
the existing finite 45-second total deadline. Connect remains five seconds, first-byte remains 15
seconds, the Route bootstrap deadline remains 45 seconds, and `max_attempts=1` remains unchanged.
The same test-only profile retains an already-idle pooled connection for at most 45 seconds; no
production profile, proxy/TUN setting, Endpoint mapping, or decoder behavior changes.

The non-live timeout-shape check proves that configuration before any network activity. The
authorized full-path revalidation then completed without a stop condition:

| Probe | Safe result |
|---|---|
| Target `A`, non-streaming, through full P3 aggregation | PASS |
| Target `B`, non-streaming, through full P3 aggregation | PASS |
| Target `A`, SSE, through full P3 aggregation | PASS |
| Target `B`, SSE, through full P3 aggregation | PASS |

The ignored target command completed successfully in about 13 seconds. It retained only its
opaque target labels and pass/fail result; no Endpoint, credential, model, request/response body,
SSE frame, or provider diagnostic was written to tracked evidence. This accepts the real-test
compatibility result locally. The final P3/G3 status record still requires the ordinary local
review/full gate and its GitHub verification; P4 remains untouched.

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
