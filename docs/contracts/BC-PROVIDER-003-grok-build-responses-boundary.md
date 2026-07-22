# BC-PROVIDER-003 Grok Build Responses request and bounded decode boundary

| Field | Value |
|---|---|
| Contract | `BC-PROVIDER-003` |
| Task | `P6-03` |
| ADR | [ADR-0044](../adr/ADR-0044-grok-build-responses-boundary.md) |
| Status | `IN_PROGRESS` — `CR-P6-03-008` adds reviewed known OAuth credential-source adaptation before its separately registered T13/T14 local-CLI-cache matrix; T1-T12 remain permanently closed |
| Domain | Fixed OAuth Build request, exact egress handoff, non-streaming/SSE decoding, and safe error syntax |

## Preconditions and bounds

1. `GrokBuildResponsesRequestBuilder` receives a currently usable P6-01 `GrokBuildCredential`, a
   selected non-empty upstream model, a representable Canonical Request, and an explicit Responses
   mode. The credential may originate from the existing strict OAuth JSON/Device Code/Refresh path
   or a validated P6-03 known-source importer, but production code does not read ambient
   credentials, files, databases, proxy settings, or network configuration.
2. The only production Build URL is `https://cli-chat-proxy.grok.com/v1/responses`. A caller must
   admit exactly that URL with P2 egress policy before it can obtain a shared transport request.
   Redirects remain denied by the caller's policy.
3. Non-streaming identity/gzip responses each have independent 1 MiB compressed and decompressed
   bounds; one error body and one complete SSE record are at most 64 KiB. A limit failure is an
   `UpstreamProtocolError`, not an unbounded buffering attempt.
4. All fixtures and unit tests are synthetic. The ignored real-account boundary, when separately
   approved, must receive an explicit opaque target label, one mode, a fixed one-request cap,
   explicit network profile, exact egress policy, and operator-controlled credential source. A
   broader authorization may schedule a finite, documented set of distinct model × mode tuples,
   but each harness invocation remains one request and cannot automatically retry a tuple. It must
   not infer any value from generic environment variables or `.env`. Under `CR-P6-03-008`, the
   harness may alternatively receive one explicit absolute official-CLI cache path; it must reject
   a simultaneous JSON credential source, read at most 64 KiB into zeroizing memory, and never
   render its path or content.

## Required behavior

| Concern | Required behavior |
|---|---|
| Request profile | Use POST, JSON, OAuth `Bearer`, current `grok-shell` version/User-Agent, client identifier/mode, protocol confirmation, selected-model override, mode-specific `Accept`/content coding, and in-memory process/request/trace associations. Do not copy hop-by-hop `Connection`, invent a user/session/cache/turn identity, or render correlation values. |
| Model and request fidelity | Serialize the selected upstream model in the body and fixed model metadata. Preserve the supported Responses request subset and reject foreign or colliding extensions rather than silently dropping semantics. Exactly one extension-free plain user Text may use scalar easy-input encoding only when it decodes to the same Canonical request; every other input remains explicit array form. |
| Egress | `into_transport_request` accepts only an admitted target equal to the fixed Build URL; a mismatch is `EgressRejected/Egress`. |
| Credential source | Standard strict JSON, Device Code, and Refresh behavior remains unchanged. CPA xAI files, account credentials, and the official CLI indexed cache are bytes-only import adapters with a fixed issuer/client identity, strict RFC3339 absolute expiry, conflict rejection, and redacted diagnostics. |
| Diagnostics | `Debug` exposes counts/names only; it never renders URL, Authorization value, OAuth token, correlation value, model mapping, raw request body, response body, Tool arguments, response id, or error text. |
| Non-streaming response | Accept only identity or gzip coding within the separate bounds, then a completed Responses object with representable message/reasoning/function-call output and a valid Canonical lifecycle. Unknown/stacked/malformed coding fails closed. |
| SSE framing | Arbitrary byte segmentation is accepted. Comments/keepalive do not advance semantics. A malformed record in one `push_bytes` call leaves the externally held decoder state unchanged. |
| Strict JSON | Duplicate names at any parsed object depth are rejected, including embedded final Tool arguments. Trailing bytes and non-object protocol envelopes are rejected. |
| Tool state | Item id, call id, and name are stable from declaration through completion. Incremental and final arguments must agree after blank/empty-object normalization to `{}`. Non-object, malformed, or incomplete non-empty arguments fail closed. |
| Completion | Every `response.output_item.done` item must itself be `completed`, and a completed response's final output-item identities must exactly equal the completed declared-item set. Final Usage is emitted at most once before Canonical `ResponseEnd`. A failed response is Canonical terminal error, not success. |
| HTTP errors | A non-2xx body yields only HTTP status plus `None`, `FreeUsageExhausted`, `InvalidGrant`, `InvalidToken`, or `Unrecognized`. P6-07 alone decides state mutation/remediation. |

## Explicitly deferred behavior

This contract does not create a production HTTP client, perform retry/failover, refresh a
Credential, write SQLite, discover models, persist Billing/Quota windows, select Cache Affinity,
record Response Ownership/Reasoning Replay, or change Health/Forbidden/Unauthorized state. Those
belong to P6-02, P6-04 through P6-07, and the shared P3 transport/runtime boundaries.

## Corresponding tests

- `build_request_uses_the_current_cli_profile_and_exact_admitted_target`
- `one_plain_user_text_uses_scalar_easy_input_without_losing_semantics`
- `non_streaming_gzip_is_bounded_and_semantically_equivalent`
- `arbitrary_sse_chunks_and_non_streaming_fixture_have_the_same_semantic_projection`
- `error_envelope_is_bounded_and_does_not_retain_upstream_text`
- `stream_rejects_duplicate_json_names_and_reports_truncation_without_advancing`
- `stream_normalizes_empty_tool_arguments_and_rejects_inconsistent_tool_metadata`
- `completed_response_requires_completed_and_exactly_accounted_output_items`
- `known_absolute_expiry_sources_import_in_memory_and_redact_tokens`
- `known_absolute_expiry_sources_reject_wrong_identity_and_unsafe_expiry`
- `absolute_expiry_sources_reject_conflicts_and_out_of_range_expiries`
- `official_cli_cache_source_is_file_only_bounded_and_exclusive`
- `official_cli_cache_preflight_builds_without_network`
