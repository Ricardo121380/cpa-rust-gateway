# BC-E2E-004 Grok Official authorized one-probe

| Field | Value |
|---|---|
| Contract | `BC-E2E-004` |
| Task | `P8-07` |
| ADR | [ADR-0060](../adr/ADR-0060-grok-official-authorized-one-probe.md) |
| Matrix | `C01`、`C03`、`C31`、`G24-G27` |
| Status | `BLOCKED`; local harness safety is verified, but no Official request is authorized or sent |
| Domain | One explicitly authorized Grok Official API-key Provider-Gate E2E |

## Preconditions and bounds

1. The ignored test stays zero-network unless all dedicated P8-07 inputs are valid, including the
   exact authorization marker and `max_external_requests=1`.
2. The API key and model are supplied only to the executing process. The test never searches local
   files, OAuth caches, server configuration, provider configuration, or environment proxy state.
3. One run admits exactly one opaque target label and exactly one mode: `non_streaming` or `sse`.
   It never selects, enumerates, retries, refreshes, fails over, or executes the other mode.
4. The request is fixed to a small plain-text prompt with a finite output limit. The native adapter
   uses only the fixed Official Responses endpoint, DNS-pinned direct egress, denied redirects,
   one pooled connection, and finite timeouts.

## Required behavior

| Concern | Required behavior |
|---|---|
| Default execution | The live test is ignored; normal test execution performs no DNS or HTTP. |
| Authorization failure | Missing/invalid authorization or cap stops before probe preparation and before any network operation. |
| Success | The single event sequence contains `ResponseStart`, at least one `TextDelta`, and `ResponseEnd`, with no `StreamError`. |
| Failure | Stop after the single attempt and report only a fixed safe category; do not infer cause or alter configuration. |
| Evidence | Retain only opaque label, selected mode, elapsed lifecycle-safe outcome, and command result. Never retain secret/model value, headers, URL variant, bodies, account, or generated text. |
| Provider isolation | The probe neither reads nor changes Grok Build/Web or Kiro state; P7 Kiro OAuth is not a P8 prerequisite. |

## Corresponding tests

- `missing_authorization_stops_before_external_configuration_is_read`
- `invalid_request_cap_stops_before_preparation`
- `complete_synthetic_configuration_prepares_without_network`
- `authorized_official_probe_uses_one_target_one_mode_and_one_send` (ignored, live only)
