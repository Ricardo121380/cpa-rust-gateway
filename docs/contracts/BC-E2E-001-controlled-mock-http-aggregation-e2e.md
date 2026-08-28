# BC-E2E-001 Controlled Mock HTTP aggregation E2E

| Field | Value |
|---|---|
| Contract | `BC-E2E-001` |
| Task | `P3-09` |
| Status | IN_PROGRESS |
| Domain | Local composition proof for the bounded OpenAI Responses aggregation path |

## Preconditions

- A Snapshot-authenticated Client Key resolves an allowed Alias or Public Model to one active Route.
- The Route has two explicit, hard-eligible OpenAI-compatible Candidates, each with one active
  Endpoint-local Credential binding.
- Each controlled peer is admitted only by a synthetic P2 `EgressPolicy` for its exact Host, port,
  scheme, and loopback CIDR. No ambient proxy, arbitrary local address, deployed URL, or real
  Credential is available to the test.
- The Actix boundary creates one finite Canonical stream and shares its cancellation/FSE gate with
  the routed executor before an upstream Attempt begins.

## Required behavior

| Concern | Required behavior |
|---|---|
| Route handoff | Snapshot resolution passes the selected Route ID, public response mode, original Canonical request, and downstream retry gate into `ResponsesExecutor::execute_routed`. Legacy executors retain the old default handoff. |
| Round-robin | Repeated equal-priority requests to the public Alias alternate across the two explicit Candidates. Each peer receives only its own configured upstream model; responses expose the stable Public Model, never the Alias or upstream model. |
| Pre-semantic 5xx | A peer's 5xx before it can produce `ResponseStart` is classified as one retryable ServerError. The Attempt loop records it, excludes that binding, and starts the explicit second Candidate within the Route budget. |
| SSE cancellation | Once an SSE Attempt has supplied `ResponseStart`, dropping the unconsumed gateway body cancels the bounded stream and drops the live upstream response. It does not create a fallback Attempt or fabricated completion. |
| Event correlation | The same Request ID correlates the HTTP Request record, one terminal Attempt record per real peer call, and final Usage only when the finite Canonical stream accepts it. |
| Bounds and privacy | Test request, JSON response, and SSE frame buffers are finite. Diagnostic surfaces and retained test records omit Client Key text, Credential text, URL, headers, and bodies. |

## Failure semantics

| Condition | Result |
|---|---|
| Route context absent at the P3 executor | Existing `RouteNotFound/Model`; no implicit model-name-to-Endpoint fallback. |
| Transport failure before a response | Existing retryable `EgressUnavailable/Egress` classification through the Attempt Driver. |
| 429 before `ResponseStart` | Existing retryable `ProviderRateLimited/Provider` classification with binding-scoped cooldown behavior from P3-06. |
| 5xx before `ResponseStart` | Existing retryable `ProviderTransient/Provider`; only explicitly configured Candidates may be tried. |
| Malformed, oversized, or incomplete controlled response before `ResponseStart` | Existing retryable bootstrap-truncation path; no raw peer diagnostic reaches the client or event sink. |
| Cancelled SSE body | Existing `Cancelled/Request` downstream path; no retry or synthetic `StreamError`. |

## Corresponding tests

- `round_robin_reaches_each_controlled_http_upstream`
- `pre_semantic_http_5xx_fails_over_to_the_second_upstream`
- `dropping_the_gateway_sse_body_closes_the_live_mock_upstream_attempt`
