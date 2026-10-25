# BC-PROTOCOL-003 Messages input-token counting

| Field | Value |
|---|---|
| Contract | `BC-PROTOCOL-003` |
| Task | `P5-02`; #68 implementation amendment (2026-10-25) |
| ADR | [ADR-0035](../adr/ADR-0035-exact-token-count-capability.md); limited successor [ADR-0127](../adr/ADR-0127-cpa-compatible-token-counting.md) |
| Status | `DONE` (local implementation; external qualification separate) |
| Domain | Messages count admission, source provenance and non-inference execution |

## Boundary

```text
POST /v1/messages/count_tokens
  -> existing client-key authentication and bounded duplicate-safe Messages decode
  -> Snapshot public-model / Alias / exact upstream model resolution
  -> CountTokensExecution { canonical request, native input, approved route + exact snapshot }
  -> existing Router candidate/credential selection without inference orchestration
  -> applicable first-party counting API OR documented local CPA estimator
  -> InputTokenCount { ProviderReported | ExactLocal | Estimated }
  -> {"input_tokens": u64} + fixed non-secret method header OR safe Anthropic error
```

## Invariants

1. Estimates only construct `InputTokenCount::Estimated`. `ExactInputTokenCount` and
   `ExactTokenCountAdapter` retain the historical exactness requirement. No exact capability is
   inferred from a model name. `ProviderReported` is provenance, not a future Usage guarantee.
2. The production RuntimeFactory installs the routed count executor in the same generation as
   HTTP authentication and routing. The default unsupported executor remains for embeddings
   without a configured count implementation.
3. The success JSON has exactly `input_tokens`; the optional diagnostic header
   `x-cpar-token-count-method` contains only a fixed method label. No prompt, tool content,
   credential, endpoint secret or raw upstream diagnostic is emitted.
4. Authorization, Alias and exact-model filtering happen before counting. Selection must use
   the exact admitted snapshot, current generation, candidate eligibility, health/quota state and
   a live credential lease. No database lookup or control-plane mutation is added to selection.
5. The applicable Anthropic first-party origin uses its count API. Third-party supported paths
   use the frozen CPA O200kBase estimator. Upstream failure, unsupported first-party paths and
   missing counts never fall back to estimation or a fabricated zero.
6. The count implementation supports bounded text/system, ordinary tools and tool history.
   Images and unsupported/unknown content fail explicitly, including nested tool results.
   [The support document](../support/messages-count-tokens.md) freezes model/input scope,
   256 KiB request bound, 64 KiB upstream response bound, algorithm and limitations.
7. Count requests never start inference or emit inference Request/Attempt/Usage events, and
   never create billable ledger entries. The upstream counting API is the only network operation
   in the native path; estimated paths make no upstream request. Cancellation drops native
   credential/egress leases; CPU work keeps its capacity until completion.
8. Bad authentication, invisible/invalid models, malformed/duplicate/streaming/oversized inputs,
   unavailable methods and upstream faults retain safe protocol errors. Debug output redacts
   input and secrets. Existing inference, billing and exact-capability behavior remains separate.

## Historical exact seam

P5-02 introduced only the exact capability and default unsupported route; its original evidence
remains [the P5-02 report](../reports/p5-02-exact-token-count-capability.md). ADR-0127 authorizes
this implementation amendment without weakening the exact capability or relabeling historical tests.
No schema migration, management API, Provider qualification or actual-client support is added here.

## Corresponding tests

- `exact_capability_never_falls_back_to_an_estimate`
- `count_tokens_returns_only_an_exact_value_and_passes_snapshot_route_identity`
- `count_tokens_rejects_default_unsupported_capability_without_an_estimate`
- `count_tokens_rejects_a_body_over_the_inference_limit_with_an_anthropic_error_envelope`
- `count_provenance_and_strict_upstream_response`
- `count_segments_preserve_cpa_order_and_reject_unsupported_content`
- `count_origin_is_exact_and_never_falls_back_on_first_party_path`
- `count_estimates_match_measured_cpa_v8_fixtures`
- `count_production_http_native_and_estimated_never_infer_or_bill`
- `count_upstream_failure_never_estimates_or_returns_fabricated_zero`
- `count_denied_routes_and_exact_model_filter_fail_before_transport`
- `count_stale_snapshot_and_cancelled_native_future_release_capacity`
