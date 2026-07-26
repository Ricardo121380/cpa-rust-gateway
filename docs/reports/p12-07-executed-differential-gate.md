# P12-07 executed differential gate report

| Field | Value |
|---|---|
| Task | `P12-07` |
| Supersedes | [P11-01 clean-room differential Fixture Harness](p11-01-differential-fixture-harness.md) |
| Package | [`tests/differential`](../../tests/differential/) (`differential-gate`) |
| Corpus location | [`tests/differential/fixtures/`](../../tests/differential/fixtures/) |
| Frozen sources | CPA `v7.2.80`; grok2api `v3.0.0` / `ec6cddca7`; Kiro-RS `c49c75e` |

## Defect closed

P11-01 compared two fields of the same committed JSON file, so it proved only that a hand-written
file is self-consistent. No gateway code ran.

The gate now computes the gateway side. `differential_gate::observe` drives this repository's real
types per subject and returns a projection; the fixture records the projection the gateway is
expected to produce, and a mismatch is a hard failure.

## What each side means

| Side | Origin | Why |
|---|---|---|
| `reference_projection` | Recorded clean-room observation | CPA, grok2api, and Kiro-RS cannot be executed here; P11-01 is explicitly an offline corpus. |
| `expected_gateway_projection` | Committed expectation, checked every run | Pins the gateway behavior so drift fails even when the two sides stay unequal. |
| Observed gateway projection | Computed by `differential_gate::observe` | The only value used for the compatible/intentional decision. |

## Subject probes

| Subject | Real code driven | Markers |
|---|---|---|
| `canonical-lifecycle` | `gateway_core::CanonicalEventState` and `CanonicalResponse` | `response-start`, `text-delta`, `response-end` |
| `configuration-authority` | `gateway_store::control_plane::SqliteControlPlaneRepository` in-memory activation | `versioned-sqlite-snapshot` |
| `provider-pool-isolation` | `GrokBuildCredentialSqliteStore` plus `GrokWebCredentialSlot`, `GrokWebBrowserEgressSession`, `GrokWebConversationState` | `build-web-pool-separation`, `browser-egress-bound-conversation` |
| `web-tool-default` | `provider_grok::GrokWebToolEmulation::default()` | `tool-emulation-default-disabled` |
| `endpoint-policy` | `provider_kiro::endpoint_policy::KiroEndpointPolicy` for both IDE and CLI | `cli-ide-endpoint-policy` |
| `event-stream-integrity` | `KiroEventStreamDecoder` with good/corrupt CRCs plus `KiroEventSemanticMapper` at nine chunk sizes | `event-stream-crc-validation`, `chunk-invariant-canonical-events` |

## Honest limits

`file-watcher-authority` is `ReferenceOnly`: this repository has no configuration-file watcher, so
no execution can produce it. A fixture that expects the gateway to emit it fails with
`unobservable_gateway_marker` rather than passing, which makes `compatible` structurally
impossible for `configuration-authority`.

A probe that cannot complete returns `ProbeError`; the gate maps it to `gateway_probe_unavailable`
and rejects the fixture. A fixture whose gateway side cannot be computed is never classifiable as
compatible.

The gate still does not prove live reference behavior, provider quota or account state, deferred
P7/P8 external authentication, performance, or release readiness.

## Value-free discipline retained

Closed enums with `deny_unknown_fields`, fixed reference/subject and subject/marker pairing, an
8 KiB fixture cap, forbidden body-bearing field names, strictly ascending duplicate-free
projections, and category-only errors are all preserved. The probes construct only synthetic
opaque identifiers; nothing recorded from an upstream enters the repository.
