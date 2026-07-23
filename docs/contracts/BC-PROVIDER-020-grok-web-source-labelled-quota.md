# BC-PROVIDER-020 Grok Web source-labelled quota observations

| Field | Value |
|---|---|
| Contract | `BC-PROVIDER-020` |
| Task | `P9-06` |
| ADR | [ADR-0066](../adr/ADR-0066-grok-web-source-labelled-quota-observations.md) |
| Matrix | `C31`、`C33`、`C34`、`D28`、`E27-E29` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P9-LOCAL-001`; zero-network |
| Domain | Synthetic REST/gRPC-Web quota observations bound to one Web egress session |

## Preconditions and bounds

1. Input is either the synthetic REST shape `{tier,window}` or synthetic gRPC-Web shape `{quota:{tier,window}}`; all objects have an exact field set and strict duplicate-key rejection.
2. A fixture is at most 64 KiB. Tier and raw window-type labels are non-empty visible ASCII and at most 128 bytes. A window has a known coarse kind or explicit `provider_defined` kind.
3. `total` and `window_seconds` are non-zero, `remaining <= total`, observation time is non-negative, and reset is later than observation.
4. State construction and every update receive an exact unexpired `GrokWebBrowserEgressSession` and caller-supplied non-negative time.

## Required behavior

| Concern | Required behavior |
|---|---|
| Source isolation | Retain observations by exactly `(REST or gRPC-Web source, window kind)`; a source may not overwrite another source's same kind. |
| Confidence | Every accepted value is `Observed`, meaning a local provider-reported projection only; it is not billing, entitlement, routing, or cross-source truth. |
| Lifecycle binding | Account reference, SSO lineage, credential revision, expiry, and egress-session ID must all match the state binding. Mismatch or expiry rejects without mutation. |
| Ordering | An older or byte-equivalent same-time source/window observation is ignored. A distinct same-time value is rejected; a newer value replaces only its exact source/window key. |
| Input safety | Unknown, missing, duplicated, cross-shape, oversized, unsafe, impossible, or malformed fixture content fails closed without retaining values. |
| Diagnostics | Tier, raw window type, account, lineage, and egress-session values redact in `Debug`; errors are value-free categories. |
| I/O boundary | This module has no HTTP, gRPC-Web frame transport, browser, Cookie, proxy/TUN, DNS, TLS, filesystem, server, scheduling, or production-configuration operation. |
| Ownership | P9-07 owns 403 evidence attribution; P9-09/G9 owns any approved live quota protocol validation. |

## Corresponding tests

- `rest_and_grpc_web_quota_windows_remain_source_and_window_isolated`
- `stale_or_conflicting_observations_and_wrong_or_expired_sessions_do_not_mutate_state`
- `malformed_cross_shape_or_unsafe_quota_fixtures_fail_closed_and_diagnostics_redact_values`
