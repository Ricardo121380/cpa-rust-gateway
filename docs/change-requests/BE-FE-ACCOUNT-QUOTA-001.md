# BE-FE-ACCOUNT-QUOTA-001 — bounded account usage reads

Status: local implementation; real-provider acceptance and release pending.

Extend existing protected getCredentialMetadata with nullable live_quota and live_quota_error. AccountQuotaObservation exposes only channel, observation time, recognized windows, used percentage and explicit reset/duration. Missing windows never become 0% or fabricated 5h/weekly periods. No opaque account IDs, tokens, balances or grant changes. Codex and Claude OAuth use fixed server-owned usage URLs, exact eligible leases and existing egress policy. No generic URL proxy. Bounded 8s/64KiB network read, 10s management request, existing four-slot admission, generation guard, five-minute per-generation success cache. API keys never use Claude OAuth usage.

The authoritative schema was changed before sync-contract. No new endpoint, browser secret, schema migration or runtime quota enforcement. Unsupported/unconnected, conflict and read failure remain distinct from observed quota (unsupported vs disconnected are currently combined and remain a follow-up).

Source protocol evidence: upstream Management Center src/utils/quota/constants.ts and features/quota/providers/{claude,codex}/data.ts, inspected 2026-09-27. These private account endpoints require live validation; imported plan labels are not live quota.
