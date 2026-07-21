# ADR-0031: Default-deny log redaction and bounded body sampling

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-21` |
| Task / Matrix / Contract references | `P4-09`; `G01`, `G03`, `G15`, `G22`, `K08`; [BC-OBS-004](../contracts/BC-OBS-004-default-deny-log-redaction-and-body-sampling.md) |

## Context

P4-08 exports only the pre-sanitized lifecycle event contract. It deliberately has no request or
response bodies, raw headers, URL, Client Key, Credential, Endpoint, or model label. Operators may
still need bounded request/response shape evidence while diagnosing a controlled issue, but copying
raw bytes or headers into `tracing` would expose upstream Secrets, Client Keys, cookies, or request
content and would put unbounded work in a sensitive path.

The P4 event writer already gives logs a background ownership boundary. P4-09 must preserve that
boundary while making the default configuration proveably retain no body byte sequence at all.

## Decision

- `gateway-observability::LogRedactionPolicy::default()` disables body sampling. It emits a stable
  `disabled` omission state rather than a partial body, header value, URL, target, Client Key, or
  Credential.
- An operator must construct a validated `BodySamplingPolicy::try_sampled` explicitly. Sampling is
  deterministic from only `RequestId`, has a numerator/denominator ratio, and bounds parsing to at
  most 16 KiB. It never samples based on a Client Key, Credential, Endpoint, URL, header, or body
  content.
- A selected body is accepted only for one unambiguous `application/json` or `application/*+json`
  `Content-Type`; duplicate, unknown, oversize, invalid UTF-8, malformed JSON, or non-JSON inputs
  fail closed to a safe omission reason. No partial byte prefix is logged.
- Header logging retains only a fixed `Content-Type` class and valid numeric `Content-Length`.
  Authorization, Cookie, API-key, proxy-authentication, malformed values, and duplicate safe
  headers increment a redacted count. All other headers increment an omitted count without keeping
  a name or value.
- Selected JSON is recursively redacted: sensitive field names, secret-like object keys, and
  recognizable authorization/token/key/secret patterns are replaced before serialization. A safe
  `SanitizedHttpLogRecord` owns only its fixed schema, Request ID, fixed direction, optional
  status, header summary, and this redacted projection. Its `tracing` helper accepts only that
  record and must run in a background observation path.
- The mechanism is a bounded Secret-redaction control, not a general data-loss-prevention system.
  Arbitrary non-secret body text can remain only after an explicit operator sampling decision;
  default operation retains no body text. Expanding to raw previews, other MIME types, or a new
  external logging transport requires a later Change Request.

## Consequences

Normal operation cannot accidentally persist a raw request/response body through this API. When an
operator enables a sample, logs retain only selected bounded JSON after redaction, while every
non-accepted condition produces a visible safe reason. Diagnostics remain useful without silently
turning a malformed or oversized body into a partial leak.

The redaction policy is transport-neutral and does not add an HTTP handler, Provider call, SQLite
schema, second event receiver, external exporter, or request-path wait. Existing P4-08 structured
event logs remain body-free regardless of whether P4-09 sampling is enabled.

## Alternatives considered

- Always log raw bodies behind a Boolean: rejected because a single configuration mistake exposes
  sensitive contents and has no finite parsing limit.
- Copy a truncated prefix on oversize/malformed input: rejected because the prefix may itself hold a
  Secret and makes redaction/parser failures unsafe.
- Preserve arbitrary headers but redact only known values: rejected because custom header names and
  values have no reliable safe allowlist.
- Rely only on repository Secret scanning: rejected because CI cannot protect runtime payloads; it
  remains a complementary gate, not the runtime redaction mechanism.

## Validation and rollback

Regression tests prove default body/header denial, explicit bounded JSON sampling, recursive
Secret/identity redaction in JSON, safe Debug/JSON forms, non-JSON/oversize rejection, duplicate
`Content-Type` failure, deterministic selection, and invalid policy rejection. The tracked Secret
scanner and complete Gate provide repository-level evidence; no Provider request is sent.

Rollback removes P4-09's redaction module and documentation, restoring the P4-08 event-only logs.
It changes no event schema, Store migration, queue ownership, Router/HTTP response behavior,
Provider traffic, Secret storage, or P4-08 telemetry semantics.
