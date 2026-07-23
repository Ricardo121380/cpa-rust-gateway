# P6-04 Build catalog, Billing, Quota Window, and Reset state

| Field | Value |
|---|---|
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Scope | Exact-Credential catalog/Billing/Quota persistence and migration 0007; no discovery HTTP or scheduler mutation |
| References | `C33`, `G26`, `G28`; [ADR-0045](../adr/ADR-0045-grok-build-runtime-continuity.md) |

`GrokBuildRuntimeStateStore` atomically stores a complete model/Billing snapshot and keeps only a strictly newer observation. Quota windows preserve source/confidence, reset time, and distinct window kinds; local estimates cannot impersonate Billing. Two Credential test snapshots remain separate and a stale catalog/quota cannot delete or overwrite the newer view.

Verification: the reopened `p6_04_07_runtime_continuity` catalog and quota cases passed again; `gateway-store` migration up/idempotence and rollback tests passed with all seven version-7 tables. No Provider request was sent. It remains locally passed until the single new P6 Delivery Gate.
