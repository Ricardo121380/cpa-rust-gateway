# P6-05 Tenant-isolated Build cache identity and affinity

| Field | Value |
|---|---|
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Scope | Versioned HMAC cache identity, exact affinity, and durable break evidence |
| References | `F10`, `F16-F18`, `G25`; [BC-PROVIDER-004](../contracts/BC-PROVIDER-004-grok-build-runtime-continuity.md) |

The Provider derives `grok-build-cache:v1:` identities from a caller-held tenant secret, Client Key, upstream model, and Canonical cache key. Build request construction refuses a raw cache key; the synthetic profile test proves the raw fixture value is absent from the resulting body. Affinity is exact to client/model/derived identity, and changing Credential or egress requires a transaction that first records the old/new relationship and cache-loss estimate.

Verification: the P6 continuity test proves two Client Keys cannot read one another's affinity, missing break evidence rejects rebinding, and a successful rebind leaves one durable break row.
