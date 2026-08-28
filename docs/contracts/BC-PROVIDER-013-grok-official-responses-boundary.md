# BC-PROVIDER-013 Grok Official text-only Responses boundary

| Field | Value |
|---|---|
| Contract | `BC-PROVIDER-013` |
| Task | `P8-02` |
| ADR | [ADR-0055](../adr/ADR-0055-grok-official-responses-boundary.md) |
| Matrix | `C01`、`C03`、`C31`、`G24`、`G27` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P7-DEFER-002`; no Official E2E has run |
| Domain | Isolated API-key Responses request, bounded text-only JSON/SSE semantics, and safe adapter vertical slice |

## Preconditions and bounds

1. The caller supplies the already selected Official API key, upstream model, execution mode, and
   injected transport. This boundary reads no database, environment, file, OAuth cache, browser
   session, proxy setting, or server account.
2. The sole production target is `https://api.x.ai/v1/responses`. It is a `POST` with exact-target
   DNS-pinned admission, `Accept`, `Accept-Encoding: identity`, `Authorization: Bearer`, and JSON
   content type. No Build CLI or Web-session headers are copied.
3. A completed JSON body is at most 1 MiB. An error body is drained only up to 64 KiB; each complete
   SSE record is at most 64 KiB. Strict parsing rejects duplicate JSON names recursively, trailing
   bytes, malformed UTF-8/framing, ambiguous records, and unrepresentable protocol objects.
4. This local task does not authorize a real xAI API call. `P8-07` / `BC-E2E-004` own the separately
   authorized Official E2E; `CR-P7-DEFER-002` makes that P8 prerequisite independent of P7/G7.

## Required behavior

| Concern | Required behavior |
|---|---|
| Provider isolation | Provider ID is exactly `grok.official`; API key, endpoint/profile, decoder, adapter, and future state are separate from Build OAuth and Web SSO. No Provider-private cross-crate reuse. |
| Request subset | Accept only extension-free text messages in `developer`, `system`, `user`, or `assistant` roles. Preserve ordered message/content semantics in explicit Responses items. Reject unsupported roles, opaque content, Tool history/results, Tool declarations, Thinking, cache fields, and all extensions before sending. |
| Egress | `into_transport_request` accepts only the exact admitted Official Responses URL. Production transport uses the injected shared DNS-pinned pool after that check. |
| Non-streaming response | Accept only a completed object with a valid response ID and completed assistant message output containing only `output_text`. Emit validated Canonical ResponseStart, MessageStart, TextDelta, optional final UsageDelta, MessageEnd, and ResponseEnd. |
| SSE response | Support `response.created`, `response.in_progress`, message item add/done, output-text content add/done, text delta/done, `response.completed`, `response.failed`, and terminal `[DONE]`. Arbitrary valid byte chunking yields the same Canonical event sequence. Tool/Reasoning/Search events fail closed. |
| Failure boundary | A non-2xx response, wrong content type, malformed non-streaming body, or pre-start stream failure is `UpstreamProtocolError/Provider`. After ResponseStart, a decoder/body failure yields exactly one `StreamError`; subsequent pulls yield no duplicate terminal event. |
| Status ownership | P8-02 does not infer status meaning, retain error text, mutate Credential/Quota/Health state, retry, fail over, schedule, persist billing, or make a real request. P8-03 owns status/header/quota/billing semantics. |
| Diagnostics | `Debug` never exposes URL, API key, Bearer header, selected model, message text, request body, response body, response ID, or upstream error text. |

## Explicitly deferred behavior

P8-03 owns Rate/Quota/Reset/Billing data. P8-04 owns Tool, Reasoning, Search declaration and
conversion. P8-05 owns Official/Build state and fault isolation. P8-06 owns differential, load,
and error matrix closure. Real Official E2E remains separately authorized through P8-07 and is not
blocked by P7/G7.

## Corresponding tests

- `request_is_fixed_authenticated_post_text_only_and_redacted`
- `non_streaming_text_fixture_runs_through_official_adapter`
- `sse_text_fixture_is_chunk_invariant_and_runs_through_adapter`
- `cache_opaque_and_unsupported_roles_are_rejected_before_transport`
- `pre_start_error_is_generic_and_post_start_failure_is_one_stream_error`
- `response_failed_is_terminal_and_opaque_search_output_fails_closed`
