# P3-01 OpenAI-compatible Responses request assembly report

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P3-01` |
| Matrix / behavior | `C16`, `D10/D11`, `L06`; Behavior 3/17/20 |
| Date | `2026-07-20` |
| Branch | `codex/p3-01-openai-responses-request` |
| Rust | `1.97.1` |
| Result | PASS locally and for the implementation commit in GitHub Fast/Full; verification-record acceptance pending |

## Delivered scope

- Added `gateway-upstream::EndpointUrl`, a typed, redacted Base URL plus inference-path target.
  It retains a configured `/v1` path when adding `/responses`, rejects URL user-info, query,
  fragment, percent escapes, duplicate separators and literal or encoded dot traversal before URL
  normalization can hide it. P2-09 remains responsible for per-attempt DNS/CIDR admission.
- Added the pure `OpenAiResponsesRequestBuilder` in `provider-openai-compatible`. It takes a typed
  endpoint, request-scoped bearer credential, selected upstream model, `CanonicalRequest` and
  `ResponseMode`; it returns a typed target, fixed headers and JSON body without opening a socket.
- The builder rewrites the public model to the selected upstream model, maps streaming mode to
  `stream` and `Accept`, emits `Content-Type` and request-scoped Bearer authorization, and maps
  representable Canonical messages, Tool definitions, Thinking, cache fields and scoped Responses
  extensions to an outbound Responses body.
- The official Responses `function_call_output.output` shape is constrained to a string or an array
  of input text/image/file content. The builder now preserves only those structurally supported
  forms and rejects object, scalar and malformed-array values instead of transmitting an invalid
  upstream payload.
- Added [ADR-0011](../adr/ADR-0011-openai-compatible-responses-request-assembly.md) and
  [BC-PROVIDER-002](../contracts/BC-PROVIDER-002-openai-compatible-responses-request.md), explicit
  `serde_json`/`zeroize` dependencies, crate-boundary policy, traceability and status records.

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-upstream -p provider-openai-compatible` | PASS; 12 upstream tests and 5 Provider request-assembly tests, including URL preservation, empty-user-info/traversal rejection, Header/Debug redaction and Tool-output forms |
| `cargo clippy --locked -p gateway-upstream -p provider-openai-compatible --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `./scripts/check-crate-boundaries.rb` and `./scripts/check-doc-links.rb` | PASS |
| `./scripts/check.sh fast` | PASS; full workspace format, Clippy, tests, source policy, Secret scan, crate-boundary, documentation and whitespace gates |
| `./scripts/check.sh full` | PASS; Fast gate plus pinned-tool verification, dependency policy and RustSec audit |

## Review

Review passed. It checked the scope boundary end to end: this Task adds no Store lookup, credential
decryption, DNS resolution, socket, HTTP client/pool, proxy, TLS, timeout, retry, routing, response
decoder or FirstSemanticEvent behavior. It also checked that only request-scoped credential storage
uses `Zeroizing`, while all `Debug` paths reveal neither target, header value, body nor selected
model.

The review strengthened two correctness boundaries before acceptance. First, URL parsers normalize
dot segments, and an explicit empty user-info field is indistinguishable from absent user-info in
the parsed username accessor. The final implementation therefore validates raw authority/path data
and rejects percent escapes before composition.
Second, raw Canonical Tool-result JSON is broader than the documented Responses `output` union; the
final encoder accepts only a string or minimum-recognized input text/image/file array and fails closed
for every other raw value. Tests cover those rejected forms as well as the supported rich-content
array round-trip.

## Scope and deferred work

P3-01 is pure request assembly only. It does not create an upstream Client Pool, configure
connect/TTFB/idle/total timeouts, select or isolate proxies, lease credentials, choose candidates,
perform weighted scheduling, maintain Health/Circuit state, send HTTP, parse upstream responses,
stream SSE, fail over, publish `/v1/models`, emit observability events, or use real upstream
credentials. Those remain P3-02 through P3-10.

All tests use synthetic targets, models and credentials. No deployed URL, Client Key, upstream
credential, Authorization header, request body or production traffic was read, logged or committed.

## GitHub CI

GitHub Actions run [29694311704](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29694311704)
passed for implementation commit `3e2d818`.

| Job | Result |
|---|---|
| Fast gate | PASS; completed `2026-07-19T16:12:00Z` |
| Full supply-chain gate | PASS; completed `2026-07-19T16:19:51Z` |

This completes P3-01 implementation acceptance. The separate verification-record commit must also
pass the same two jobs before P3-02 can begin.
