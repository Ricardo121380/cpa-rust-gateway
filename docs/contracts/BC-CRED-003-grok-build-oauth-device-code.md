# BC-CRED-003 Grok Build OAuth credential and Device Code

| Field | Value |
|---|---|
| Contract | `BC-CRED-003` |
| Task | `P6-01` |
| ADR | [ADR-0042](../adr/ADR-0042-grok-build-oauth-credential-boundary.md) |
| Status | `DONE` |
| Domain | Bounded OAuth import, Device Authorization, and mock transport safety |

## Preconditions and bounds

1. The importer and OAuth response parser accept at most 64 KiB. Secret-like individual values
   are bounded at 16 KiB, contain no surrounding whitespace or control characters, and are retained
   in zeroizing memory only.
2. One token response must contain non-empty `access_token`, `refresh_token`, and one positive
   integral `expires_in` bounded to 366 days. `expires_at`, `expires_at_ms`, `expiration`, or
   `expires` make expiry ambiguous and are rejected. Optional `token_type` is only `Bearer` (case
   insensitive); `id_token` is validated as bounded opaque input then discarded.
3. Strict JSON rejects duplicate keys at every nesting level. Accepted token/device objects use
   only the documented structural fields; unknown fields do not silently become credential state.
4. The Device Authorization response has a non-empty device/user code, HTTPS verification URI,
   positive expiry, and optional positive poll interval. The default poll interval is five seconds;
   a `slow_down` outcome adds five seconds to the current interval.
5. All tests use committed synthetic strings and an in-process scripted transport. No test opens a
   socket, reads ambient OAuth configuration, sends a Provider request, or modifies a server.

## Required behavior

| Concern | Required behavior |
|---|---|
| Secret diagnostics | `GrokBuildCredential`, Device Authorization, OAuth request, and HTTP response `Debug` output redact secret values. OAuth error types contain only fixed classifications and never raw JSON or token text. |
| Imported credential | JSON import computes a precise expiry from caller-supplied observation time plus `expires_in`; it never infers expiry unit or trusts an `id_token` identity claim. |
| Transport boundary | A mock sees only fixed endpoint (`device/code` or `token`) and operation kind. Secret form fields are private to `provider-grok`; P6-01 creates no real HTTP/TLS/proxy client. |
| Device poll timing | Polling before `next_poll_at_ms` returns `PollingTooSoon` without invoking transport. `authorization_pending` schedules the current interval; `slow_down` schedules the increased interval. |
| Device terminality | `access_denied`, `expired_token`, a local deadline, and a granted token all terminally close the poller. A later poll returns `DeviceFlowCompleted`. |
| Refresh boundary | A refresh flow must match the Credential's public client id and returns a new credential only from a success token response. It does not coordinate concurrent refreshes, compare revisions, or persist state. |

## Failure semantics

| Condition | Result |
|---|---|
| Duplicate, malformed, or non-object JSON | `InvalidJson` with no input echo. |
| Empty/unsafe token, invalid URL, or missing required field | `InvalidField` or `MissingField`; no raw value retained in the error. |
| Ambiguous/non-integral/overflowed expiry | `AmbiguousExpiration` or `InvalidTimestamp`; no guessed timestamp. |
| Unsupported token type/unknown field | `UnsupportedTokenType` or `UnexpectedField`; no permissive fallback. |
| Early/terminal Device poll | `PollingTooSoon` or `DeviceFlowCompleted` before any new exchange. |
| Mock transport failure or invalid OAuth response | `TransportUnavailable`, `InvalidDeviceAuthorizationResponse`, or `InvalidTokenResponse`; no retry loop is created by P6-01. |

## Corresponding tests

- `strict_import_retains_only_validated_fields_and_redacts_tokens`
- `importer_rejects_duplicate_unsafe_and_ambiguous_oauth_shapes`
- `device_code_state_machine_honors_interval_slow_down_and_redaction`
- `refresh_requires_a_matching_client_and_never_exposes_form_secrets`
