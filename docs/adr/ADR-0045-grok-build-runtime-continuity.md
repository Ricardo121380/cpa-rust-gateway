# ADR-0045: Grok Build runtime state and continuity isolation

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-23` |
| Task | `P6-04` to `P6-07` |
| Matrix / Contract | `C33`, `E06-E12`, `F07`, `F10-F18`, `G24-G28`; [BC-PROVIDER-004](../contracts/BC-PROVIDER-004-grok-build-runtime-continuity.md) |

## Decision

1. Persist Build Billing plan, model capability snapshots, and each Quota Window by exact Credential. Every quota row carries its source, confidence, observation time, reset time, and raw window kind. A stale or equally timed input cannot replace a later durable snapshot.
2. Derive an upstream `prompt_cache_key` with versioned HMAC-SHA256 over tenant secret, Client Key identity, upstream model, and the Canonical cache key. Raw client cache keys are rejected at the Build request boundary; only the opaque derived value can be serialized upstream.
3. Store Cache Affinity, Response Ownership, and Reasoning Replay in independent version-7 SQLite tables. An affinity credential or egress change requires an atomic durable break record. A continuation must resolve its exact original Credential and cannot fall back silently.
4. Encrypt replay state with the existing AEAD `SecretStore` and tenant/model/session associated data. Reads deduplicate, expiry fails closed, and an explicit successful no-replay outcome can remove only its exact row.
5. Treat `401`/OAuth invalid token as Credential reauthorization, unsupported `403` as egress rejection unless independent account evidence exists, Free-usage exhaustion as its own quota window, account/provider `429` separately, and ordinary `408`/`5xx` as Provider cooldown only.

## Consequences

- Version 7 adds seven P6-only tables; it neither changes Canonical types nor reads SQLite in the stream hot path.
- Build and Web-style quota observations can coexist but cannot impersonate one another.
- A direct Build request with a raw cache key now fails before transport. Callers must derive an opaque identity using caller-owned tenant secret material.
- The state APIs are Provider-private primitives. Route scheduling, management HTTP, live quota discovery, and automatic failover remain outside this Phase.

## Validation and rollback

`p6_04_07_runtime_continuity` proves credential isolation, stale-snapshot retention, source/confidence rejection, cache-affinity breaks, exact ownership, AEAD replay non-retention, deduplication, clearing, and the status/action matrix. Existing P6-02 tests retain revision/CAS and same-Credential refresh singleflight coverage. Rollback removes migration 0007 and these Provider-private modules as one versioned change; it does not alter live accounts or server data.
