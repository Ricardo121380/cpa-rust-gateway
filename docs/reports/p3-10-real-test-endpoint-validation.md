# P3-10 real-test Endpoint validation plan

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P3-10` |
| Matrix / behavior | `C16`, `G05`, `G12-G15`, `G21`, `K03-K06`, `L20-L31`; Behavior 1/4/5/9/17/20 |
| Date | `2026-07-20` |
| Branch | `codex/p3-10-real-endpoint-validation` |
| Status | IN_PROGRESS — planning complete; no external request has been sent |

## Entry review

P3-09's final GitHub CI run
[29713118335](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29713118335) completed
successfully for commit `a9fb25d`. Therefore P3-10 is the plan's only `IN_PROGRESS` Task.

The current workspace contains no approved real-test Base URL, credential, model, proxy profile,
or request budget. No ambient API-key variable, prior chat text, local shell history, or test-only
P3-09 value is eligible to fill those fields. This plan deliberately makes zero external requests.

## Planned execution

1. Add a dedicated ignored manual integration target,
   `p3_10_real_endpoint_validation`, which composes the existing P3 path rather than changing the
   production library boundary or treating P3-09's fixture decoder as a production decoder.
2. Require an explicit opt-in switch and one complete set of local variables for each opaque target
   label. The harness will reject partial configuration before DNS/transport construction and will
   never read generic provider variables.
3. Run at most the approved count of fixed, non-sensitive `minimax-m3` public-model probes: one
   non-streaming and one SSE probe for target A, then the same for target B. Any optional two-target
   confirmation is separately counted and requires explicit approval.
4. Emit only a redacted local summary: target label, mode, status/content-type category, latency
   bucket, canonical event-shape result, public-model rewrite result, and boolean event-correlation
   result. Keep any raw troubleshooting data only under ignored `docs/reports/private/` and delete
   it after review.
5. Review the harness before a live call, run the requested local checks, then review the redacted
   evidence. A compatibility mismatch is recorded as a stopped finding, not repaired by scope creep.

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

Before execution, the operator must also explicitly confirm:

- both targets are authorized test relays using the OpenAI-compatible Responses endpoint;
- the permitted request count and maximum spend, including any two-candidate confirmation calls;
- whether traffic must use `direct` or a preconfigured SOCKS5 profile; and
- that a `401`, `403`, quota/billing response, transport failure, or unexpected response stops
  rather than triggers an automatic retry.

The placeholders above are documentation only. They are not usable configuration, and the harness
will not contact them.

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
