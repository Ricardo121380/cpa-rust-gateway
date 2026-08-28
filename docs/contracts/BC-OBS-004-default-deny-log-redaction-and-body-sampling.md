# BC-OBS-004: Default-deny log redaction and body sampling

| Field | Value |
|---|---|
| Contract | `BC-OBS-004` |
| Task | `P4-09` |
| ADR | [ADR-0031](../adr/ADR-0031-default-deny-log-redaction-and-body-sampling.md) |
| Extends | [BC-OBS-003](BC-OBS-003-single-consumer-telemetry-fanout.md) structured background telemetry |
| Domain | Bounded Secret-safe HTTP log projection |

## Entry and boundary

`LogRedactionPolicy::capture_http_record` receives caller-owned HTTP metadata only after that
caller has crossed its response-path boundary. It is not a `GatewayEventSink`, `EventQueueReceiver`,
HTTP handler, Router dependency, SQLite writer, Provider, network exporter, or body-storage API.
`try_emit_sanitized_http_log` accepts only the resulting `SanitizedHttpLogRecord`; it must run from
the background observation owner, never directly in an HTTP/Router response path.

The record contains no URL, Endpoint identity, Client Key, Credential, Cookie, Authorization value,
raw header value, raw body bytes, request/response body prefix, or Provider diagnostic text.

## Sampling and body rules

| Condition | Required safe result |
|---|---|
| Default policy | `body.status = omitted`, `reason = disabled`; no body bytes or string content are retained. |
| Explicit policy not selected by its Request ID bucket | `omitted/not_selected`; no body parse or partial capture. |
| Selected non-JSON or ambiguous/duplicate `Content-Type` | `omitted/not_json`; no body capture. |
| Selected body exceeds its configured finite bound (maximum 16 KiB) | `omitted/too_large`; no prefix capture. |
| Selected invalid UTF-8 or malformed JSON | `omitted/invalid_utf8` or `omitted/malformed_json`; parser errors are not logged. |
| Selected valid JSON | Exactly one recursively redacted JSON value can appear in the safe record. |

An enabled policy requires `0 < numerator <= denominator` and `0 < max_bytes <= 16384`.
Sampling uses only a versioned SHA-256 calculation over `RequestId`; equal Request IDs make the same
selection decision. The sample ratio, URL, Client Key, Credential, Endpoint, header values, and
body values never enter emitted labels or sample keys.

## Header and JSON redaction rules

- Header output can contain only `content_type` (`json`, `json_suffix`, or `event_stream`) and a
  valid numeric `content_length`, plus redacted/omitted counts. It retains no arbitrary header name
  or value.
- Authorization, proxy authorization, cookies, API-key forms, malformed `Content-Type`/
  `Content-Length`, and duplicate safe headers increment the redacted count. Other headers only
  increment the omitted count.
- JSON values beneath key names containing a token/key/secret/password/credential form are replaced
  with `[REDACTED]`; secret-like keys are renamed to a fixed redacted key marker.
- JSON strings containing authorization, bearer, API-key, token, client-secret, cookie, or common
  provider-key patterns are replaced with `[REDACTED]` before `Debug`, JSON, or `tracing` output.
- This contract does not claim to detect every possible arbitrary user secret embedded in an
  otherwise unmarked free-text field. Such text can appear only under explicit body sampling; the
  default policy is complete body denial.

## Record and failure invariants

- `SanitizedHttpLogRecord` has a fixed schema version, Request ID, fixed direction, optional HTTP
  status, safe header summary, and safe body sample only.
- Serializing or `Debug` formatting the record must not introduce a discarded raw value.
- A failure to serialize a safe record returns `Rejected`; it does not render a partial record,
  retry a durable event batch, wait for the producer, or send a Provider request.
- P4-08 event JSON remains independent and body-free whether sampling is disabled or enabled.

## Corresponding tests

- `gateway-observability::log_safety::tests::default_policy_retains_no_body_or_header_values`
- `gateway-observability::log_safety::tests::explicit_json_sampling_is_bounded_and_recursively_redacts_secrets`
- `gateway-observability::log_safety::tests::sampling_never_parses_oversize_or_non_json_bodies`
- `gateway-observability::log_safety::tests::duplicate_content_type_fails_closed_before_body_sampling`
- `gateway-observability::log_safety::tests::invalid_or_unselected_bodies_never_retain_a_prefix`
- `gateway-observability::log_safety::tests::sample_selection_is_stable_and_invalid_configurations_fail_closed`
- `scripts/secret-scan.sh --all`
- `cargo test --locked -p gateway-observability`
- `cargo clippy --locked -p gateway-observability --all-targets --all-features -- -D warnings`
- `./scripts/check.sh full`
