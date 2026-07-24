# BC-PROVIDER-021 Grok Web explicit Tool emulation

| Field | Value |
|---|---|
| Contract | `BC-PROVIDER-021` |
| Task | `P9-08` |
| ADR | [ADR-0068](../adr/ADR-0068-grok-web-explicit-tool-emulation.md) |
| Matrix | `C30`、`C31`、`D28`、`F17` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P9-LOCAL-001`; zero-network |
| Domain | Default-off Web Tool prompt convention and explicit `emulated` metadata |

## Preconditions and bounds

1. `GrokWebToolEmulation::default()` is disabled. Only `new(true)` enables a Tool prompt convention.
2. Enabled prompt composition accepts at most 16 extension-free Tools. Names are printable ASCII and at most 128 bytes; descriptions are printable/space ASCII and at most 1024 bytes; schemas are JSON objects up to 16 KiB; the aggregate addendum is at most 64 KiB.
3. The base request remains one extension-free user text message with no Thinking/cache/provider extensions. Tool response execution and parsing are outside this contract.

## Required behavior

| Concern | Required behavior |
|---|---|
| Default behavior | Disabled config has metadata `Disabled`, emits no addendum, and keeps a tool-free emulation-aware request byte-for-byte equal to the P9-03 builder output. |
| Disabled Tools | A request containing Tools is rejected before outbound fixture body creation; no implicit prompt injection occurs. |
| Enabled behavior | Metadata is `Emulated`; enabled valid Tools become a bounded structured `mode=emulated` addendum before the user text. The fixture body has no native `tools` field. |
| Native capability truth | Native semantic capabilities include `Streaming` only and exclude `Tools`/`ParallelTools` in both flag states. `Emulated` must not be represented as a native capability. |
| Input safety | Too many, unsafe, extension-bearing, duplicate-key, non-object-schema, oversized, or malformed Tool declarations fail closed before an addendum or outbound body is returned. |
| Diagnostics | Tool and prompt content are zeroizing/redacted in prepared-prompt and outbound-request `Debug` output. |
| I/O boundary | No HTTP, Web Tool protocol parsing, Tool invocation, browser/profile, Cookie, proxy/TUN, DNS, TLS, filesystem, server, scheduler, or production-config operation exists. |
| Ownership | P9-09/G9 owns any approved live feature-flag/canary behavior; no local fixture proves a real Web Tool convention. |

## Corresponding tests

- `default_flag_is_disabled_has_no_native_tools_and_never_injects_prompt_bytes`
- `enabled_flag_injects_only_a_bounded_visible_emulated_convention`
- `enabled_unsafe_tool_fails_closed_while_disabled_path_still_injects_nothing`
