# BC-PROVIDER-003 Grok Build Responses request and bounded decode boundary

| Field | Value |
|---|---|
| Contract | `BC-PROVIDER-003` |
| Task | `P6-03` |
| ADR | [ADR-0044](../adr/ADR-0044-grok-build-responses-boundary.md) |
| Status | `DONE` — `CR-P6-03-013` permits P6 continuation without replaying T18 or sending T19. The direct result remains `unattributed` safe 4xx; this contract does not claim direct acceptance. |
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
| Request profile | Use POST, JSON, OAuth `Bearer`, the current official CLI version and evidence-supported versioned workspace User-Agent, client identifier/mode, protocol confirmation, selected-model override, mode-specific `Accept`/content coding, and in-memory process/request/trace associations. Do not copy hop-by-hop `Connection`, invent a user/session/cache/turn identity, or render correlation values. |
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

## 2026-10-10 version-profile successor

The fixed version header and User-Agent version now both use `1.0.50`, observed in the
[official stable pointer](https://x.ai/cli/stable) and its installer-declared
[official fallback](https://storage.googleapis.com/grok-build-public-artifacts/cli/stable).
The [official changelog](https://x.ai/changelog/build) dates that release to 2026-10-06.
[Source hashes and observation](../reports/assets/cpar-build-live-20261010/public-profile-source.json)
retain the exact public inputs; the pointer hash is not an official binary hash.

The pinned [official sampler source](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-sampler/src/client.rs#L560)
documents proxy version gating and the versioned shell User-Agent. Its public source version
is 1.0.45; the [version module](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-version/src/lib.rs#L11)
separately takes the shipping version from the release pipeline. Exact stable-binary-to-source
mapping remains unproven. This update preserves the existing fixed `linux; x86_64` profile,
other headers, request semantics, egress, credential checks, and retry policy.

One real isolated Pin on the preceding `0.2.111` profile returned a safe HTTP426 rejection.
Version gating is a possible cause, not an established diagnosis. This local version-drift
repair does not claim to resolve HTTP426 or establish real inference acceptance; no real
request was replayed to validate it.

## Explicitly deferred behavior

This contract does not create a production HTTP client, perform retry/failover, refresh a
Credential, write SQLite, discover models, persist Billing/Quota windows, select Cache Affinity,
record Response Ownership/Reasoning Replay, or change Health/Forbidden/Unauthorized state. Those
belong to P6-02, P6-04 through P6-07, and the shared P3 transport/runtime boundaries.

## CPAR-38 decoded Usage and selected diagnostic boundary

`CanonicalEventSource::decoded_usage` exposes only a validated snapshot already buffered
in this attempt, without additional reads. Build JSON/SSE sources and native/runtime
wrappers forward it. The HTTP observer preserves known fields on a later projection
failure using non-additive merging and existing exact-Usage idempotence. Unread or invalid
Usage remains unavailable. No transport success or missing row establishes free usage.

Native Build Usage is not limited to the four currently mapped source counters. Unknown
root or detail fields are retained together under `grok.build.usage`, including explicit
zero/null/empty values. Without a proven target mapping the existing response projector
rejects that extension; no native price field is treated as an independently confirmed
charge. A supplied `total_tokens` must confirm checked input-plus-output arithmetic when
both counters are known. A total with a missing operand stays retained and unsupported,
without inventing the absent counter. Absent/null totals remain absent evidence.

If projection rejects the Usage event itself, that event has already left the inner
queue. The runtime preserves its validated snapshot for the existing failure flush,
without another upstream read, additive merging or a second ledger entry. A response
with an unsupported summary or Usage remains a failed Request even when its Attempt
succeeded. This tightens fidelity; it does not add a cross-protocol representation.

The opt-in private runtime transport decorator in [ADR-0105](../adr/ADR-0105-projection-failure-observation.md)
is separate from ordinary Debug/log sampling: exact source and public request hash,
Chat JSON only, one exclusive capture, anchored 0700/0600 private files, original body
bound, no outbound headers or credentials, no added read/retry/refresh. Normal diagnostics
remain value-free. Native content never enters Git, GitHub or external model input.

#59 adds [complete encrypted Usage diagnostics](BC-OBS-006-native-usage-evidence.md) through the
existing bounded writer. For unrepresented Usage, `grok.build.usage` now retains the exact received
object slice, with a closed `grok.build.usage_source` label for JSON versus SSE terminal origin.
The public projector still rejects it. The sensitive companion is excluded from ordinary events;
authorized readback, fixed TTL, commit confirmation and failure semantics follow ADR-0109.

## #61 rejection preparation (metadata admission remains blocked)

[ADR-0111](../adr/ADR-0111-build-response-rejection-integrity.md) closes the Build response root
schema around consumed semantics and validates supplied item statuses before normalization.
JSON and SSE use the same root rules. The fixed `object` tag, output/status shapes and applicable
error/incomplete details are checked; unmapped root fields fail instead of disappearing.
Created/in-progress SSE snapshots cannot contradict their phase or silently carry unconsumed
nonempty output/Usage. Existing model-identity validation still precedes these shape checks.

A native failed SSE terminal parses its already received Usage before failure. Valid counters
remain on the same Attempt; extra Usage still rejects and uses BC-OBS-006's confirmed exact
encrypted companion. No subsequent frame, request, retry or bill is needed to establish this.
Failure commits before any already-coalesced tail is interpreted, so transport chunk boundaries
cannot erase that Usage. A separately delivered tail stays unread; successful chunks keep their
strict conflict checks.
The received complete Usage object is not a claim that the whole failed response was retained.

The [field review](../reports/cpar-61-field-qualification-20261018.md) is incomplete for a shared
actual JSON/SSE field. This section grants no metadata separation, root/item/part success
retention, Messages summary omission, real alias or #61 completion.

## #61 cancellation continuation (cost remains NOT_ADMITTED)

[ADR-0112](../adr/ADR-0112-cancelled-stream-source-release.md) releases the upstream source and
actual credential lease as soon as a received terminal/error or cancellation no longer needs it.
An already pending local Usage confirmation can finish after resource release, without a second
submission or an upstream read. Validated received Usage remains Attempt-bound and unpriced;
unreceived terminal Usage stays UNKNOWN. The delivery owner keeps the Request incomplete/cancelled
even if the native Attempt started and local recording later commits.

Controlled public tests use the production compiler/factory and actual Build adapter. Only the
fixed outbound request's socket handoff is replaced with an admitted numeric loopback target;
the real UpstreamClientPool enforces idle and total deadlines. Body drop and real socket reset,
physical truncation and semantic EOF, pending standard/private receipts, and an explicit later
request establish scoped cancellation and capacity recovery. A TCP write-half-close alone is not
defined as cancellation. No cost success policy, profile/transport switch or extra read is added.

## Corresponding tests

- `public_build_disconnect_releases_upstream_before_pending_usage_receipt`
- `public_build_cancellation_and_disconnect_stop_reads_and_restore_capacity`
- `public_build_upstream_eof_idle_and_total_timeout_fail_once_without_resend`
- `public_build_evidence_conflict_fails_once_and_keeps_original_exact_usage`

- `public_build_rejects_unreviewed_root_fields_and_shapes`
- `public_build_rejects_invalid_added_item_status`
- `public_build_failed_sse_retains_received_usage_on_exact_attempt`
- `public_build_rejects_conflicting_sse_snapshot_status`
- `public_build_rejects_known_request_controls_before_any_send`
- `public_build_standard_json_and_fragmented_sse_keep_two_tool_cycles`

- `grok_build_projection_rejection_has_safe_reason_and_request_id`
- `grok_build_projection_failure_preserves_usage_and_ledger`
- `grok_build_native_summary_and_usage_public_matrix`
- `native_usage_unknown_fields_are_retained_and_fail_closed`
- `native_usage_totals_must_confirm_known_counters`
- `native_summary_is_not_a_reasoning_body_and_item_metadata_stays_strict`
- `grok_build_diagnostic_is_private_selected_and_one_shot`
- `grok_build_diagnostic_rejects_unsafe_directory_or_source`

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

## #62 summary-only request construction

[ADR-0113](../adr/ADR-0113-responses-summary-without-effort.md) and
[BC-PROTOCOL-009](BC-PROTOCOL-009-codex-responses-controls.md) define the reviewed summary-only
object. The fixed Build encoder preserves the absent effort and revalidates the closed shape;
its reserved-field protection rejects typed Thinking collisions. Existing model/lease/cache,
output/history, strict Usage and failure rules remain. Controlled encoding/delivery does not
establish production 1.0.50 service acceptance or effective default effort. Namespace,
client_metadata and hosted search qualifications remain BLOCKED; no profile/capability changes.

- `public_build_summary_only_reasoning_preserves_absent_effort`
- `public_build_summary_only_json_and_fragmented_sse_keep_two_tool_cycles`
- `public_build_codex_fields_remain_individually_blocked_before_send`
- `public_build_namespaced_output_is_rejected_without_flattening_or_retry`
