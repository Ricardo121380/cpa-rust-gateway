# BC-E2E-002 Authorized real-test Endpoint validation

| Field | Value |
|---|---|
| Contract | `BC-E2E-002` |
| Task | `P3-10` |
| Status | Accepted |
| Domain | Opt-in validation of two real OpenAI-compatible Responses test relays |

## Preconditions

- P3-09 is accepted, including the final GitHub Fast/Full status record.
- The operator has explicitly authorized external calls and supplied two non-production test
  targets, their API-format path, one test-only credential per target, the accepted upstream model
  per target, an approved network profile, and a maximum request/spend budget.
- Credentials are present only in operator-controlled local state and supplied through explicit
  `P3_10_*` variables. The process does not infer values from ambient provider variables.
- Each URL passes the configured P2 egress admission immediately before transport. A SOCKS5 profile,
  if approved, is explicit local-DNS `socks5://host:port`; an optional narrow per-target CIDR is
  required for a private/local relay. Proxy auto-discovery and TUN-rule mutation are outside this
  contract.
- The request input is a fixed, non-sensitive probe with `max_output_tokens=32`. No user prompt,
  production body, account data, tool call, file upload, or side-effecting upstream operation is
  allowed.
- Under `CR-P3-G3-001`, the fixed client-visible test model is `p3-chatgpt-compat` and its request
  alias is `p3-chatgpt-compat-alias`. They are test-only compatibility labels, not an assertion of
  either Candidate's actual upstream model identity. Each Candidate retains its separately supplied,
  private upstream model mapping.

## Required behavior

| Concern | Required behavior |
|---|---|
| Opt-in guard | Normal `cargo test`, CI, and a missing/invalid live-test switch make zero external requests. A live run needs all explicit P3-10 variables, `P3_10_LIVE_AUTHORIZATION=approved`, and exactly four approved requests. |
| Isolation | Each target receives one bounded non-streaming and one bounded SSE probe through an explicit Candidate/Endpoint/Credential mapping. The route has `max_attempts=1`; it cannot silently retry, fail over, or substitute an ambient endpoint, credential, proxy, or model. |
| Bootstrap deadline | P3-10's Route bootstrap deadline is explicitly bounded to the same 45 seconds as its configured total transport deadline. P3-09's controlled Mock target keeps its separate one-second test deadline. |
| Live transport bound | Under `CR-P3-G3-003`, only the ignored P3-10 live profile uses connect `5s`, first-byte `15s`, SSE response-idle `45s`, and total `45s`. The same test-only profile retains an already-idle pooled connection for at most `45s`; no production profile, retry, proxy, or TUN behavior changes. |
| Bounded reads | Upstream JSON, client-visible response verification, and each test-only SSE frame are independently capped at 64 KiB. A frame at the limit is accepted; a larger frame is safely rejected without retaining or rendering its raw bytes. |
| Protocol proof | A successful non-streaming probe has an admitted HTTP target, a successful `2xx` status/content type, and a bounded canonical `ResponseStart` to `ResponseEnd` sequence. A successful SSE probe has a successful `2xx` status, the expected stream content type, starts before output consumption, and terminates through the same bounded canonical path. |
| Public boundary | Client input uses the fixed test-only `p3-chatgpt-compat` public model and the Snapshot-authenticated project boundary. Client-visible output must use that alias; upstream model, endpoint identity, and credential never escape the response or retained evidence. |
| Event correlation | Each successful probe has a distinct opaque test Request ID and proves its Request, terminal Attempt, and Usage observations share that one internal correlation without disclosing any value. |
| Privacy | Console and tracked reports retain only opaque labels and safe summaries. URL, headers, key material, request/response/SSE payloads, provider response IDs, raw trace, and upstream model are forbidden from tracked artifacts. |
| Stop conditions | An authorization, billing, protocol, timeout, network, safety, or redaction anomaly stops further external probes. No automatic retry or decoder broadening is allowed. |

## Failure semantics

| Condition | Result |
|---|---|
| Missing explicit live configuration or approval | Harness skips/fails before construction of an outbound request; zero network traffic. |
| Egress rejection or proxy-profile mismatch | Redacted local failure, no bypass and no TUN/proxy configuration change. |
| 401/403, quota/billing warning, 429, 5xx, timeout, malformed response, or unexpected content type | Stop the target's remaining probes; retain only a safe category/status summary and mark P3-10 unresolved. |
| First semantic event followed by stream failure | Preserve the P3-06 no-transparent-replay rule; record a safe failure category only. |
| Raw data would be needed to diagnose a failure | Store it, if at all, solely in the ignored private evidence location; do not commit or copy it into a tracked report. |

## Corresponding tests

- Ignored target: `cargo test --locked -p gateway-http-actix --test p3_10_real_endpoint_validation -- --ignored --nocapture`.
- Non-live guard coverage: absent authorization, incorrect request cap, direct/SOCKS5 profile
  isolation, private-CIDR parsing, complete synthetic configuration, and client-boundary redaction
  all execute without DNS or transport activity. The shared harness also proves the 64 KiB SSE
  frame boundary accepts data at the cap and rejects data above it without appending the excess.
- Existing P3-03 scheduler distribution tests remain the proof for high-volume fairness; P3-10 does
  not generate 1000 paid external requests.
- Existing P3-09 local E2E remains the deterministic proof for retry/failover/cancellation branches;
  P3-10 only records the authorized real-target compatibility outcome.
