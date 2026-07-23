# BC-PROVIDER-009 Kiro dynamic capability and last-success snapshots

| Field | Value |
|---|---|
| Task | `P7-06` |
| ADR | [ADR-0051](../adr/ADR-0051-kiro-dynamic-capability-snapshots.md) |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |

| Concern | Requirement |
|---|---|
| Ownership | One success belongs to exactly one `CredentialId`. A discovery failure, replacement, or freshness result for one Credential cannot alter another Credential's models or subscription projection. |
| Input | An injected probe provides paired, bounded model and subscription observations. The boundary validates JSON structure, model IDs/counts, optional token limits, timestamps, duplicate conflicts, and subscription field types; it retains no bodies, raw title, URLs, tokens, or error text. |
| Subscription | Only `Free`, recognized `Paid`, or `Unknown`, and supported/unsupported/unknown overage survive parsing. Unknown values do not claim paid entitlement. |
| Snapshot | A complete paired success atomically replaces only the same Credential's previous success, uses a positive target-local version, and rejects a timestamp earlier than its retained success. |
| Freshness | P4-02 policy is explicit: Fresh through 6h, Stale through the 72h hard expiry, and refresh-due at 24h by default. Pre-observation clocks and deadline overflow fail safely. |
| Failure fallback | A failed probe is non-mutating. It may contribute only its same-Credential Fresh or Stale last success; Expired data is not admitted. No raw failure is saved. |
| Union | Current and eligible-stale Credential results are deterministic and deduplicated by exact source model ID. No synthetic `-thinking` model is emitted. Conflicting token limits collapse to unknown, never a larger limit. |
| Isolation | A failed or empty Credential must not prevent a different successful Credential from contributing. If none can contribute, return an empty safe projection and unavailable count rather than a probe error body. |
| Deferred | HTTP, API-key/OAuth injection, endpoint fallback/retry, real probes, account/quota/429 classification, scheduler mutation, public model endpoints, and Tool/Thinking transformation remain outside P7-06. |
