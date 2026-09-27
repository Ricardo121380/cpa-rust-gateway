# BE-FE-ACCOUNT-QUOTA-001 — bounded account usage reads

Status: local implementation; real-provider acceptance and release pending.

Extend existing protected getCredentialMetadata with nullable live_quota and live_quota_error. AccountQuotaObservation exposes only channel, observation time, recognized windows, used percentage and explicit reset/duration. Missing windows never become 0% or fabricated 5h/weekly periods. No opaque account IDs, tokens, balances or grant changes. Codex and Claude OAuth use fixed server-owned usage URLs, exact eligible leases and existing egress policy. No generic URL proxy. Bounded 8s/64KiB network read, 10s management request, existing four-slot admission, generation guard, five-minute per-generation success cache. API keys never use Claude OAuth usage.

The authoritative schema was changed before sync-contract. No new endpoint, browser secret, schema migration or runtime quota enforcement. Unsupported/unconnected, conflict and read failure remain distinct from observed quota (unsupported vs disconnected are currently combined and remain a follow-up).

Source protocol evidence: upstream Management Center src/utils/quota/constants.ts and features/quota/providers/{claude,codex}/data.ts, inspected 2026-09-27. These private account endpoints require live validation; imported plan labels are not live quota.

## 2026-09-27 Kiro extension

`source` adds `kiro`; optional nullable email/plan are allowlisted display evidence. Windows add nullable used/limit/unit. `partial` explicitly marks omitted/uninterpreted quota components, including trial/bonus information. The frontend must not present these base resource counters as the complete remaining balance. No new operation; generated contract follows sync-contract. CLI metadata targets are distinct from discovery support. Canonical preparation permits only the regional metadata host, with revisioned repair of CPAR-owned old policies. Custom egress remains operator-owned.

References: https://github.com/QiXia881/xkiro.rs/blob/master/src/kiro/endpoint/ide.rs and https://github.com/QiXia881/xkiro.rs/blob/master/src/kiro/endpoint/cli.rs (third-party implementation; live validation pending).

## 2026-09-27 native quota and catalog follow-up

Add protected `GET /admin/native-accounts/{account_id}/usage?revision=`. Authentication remains mandatory; opaque native credentials never reach the browser. Read before/after revision checks reject late success and failure after account mutation with 409. The result is an observation or a closed error code; no-store, four-slot admission, 20s request bound, fixed provider destinations, bounded response bodies, no inference or retry fallback. Build uses OAuth billing credits; Console exchanges SSO for DPoP and signs its usage request; Web reads REST windows and an optional bounded gRPC-Web Credits aggregate. `used_percent` is nullable for unknown percentages/zero denominators. Web remains explicitly partial; media pools and product breakdowns are not represented.

Native list and unified account projection add optional nullable `adapter_models`. For Web/Console this comes from the same tables used by the actual request adapters; it is not upstream discovery, account entitlement, or a serving grant. Build remains on its dynamic directory path. Frontend list/detail quota queries share exact account/revision keys, cancellation and two concurrent readers. Unknown or failed metadata never becomes zero.

Kiro active trial/bonus and observed overage counters are separately projected, bounded to six windows; malformed/omitted/truncated components retain `partial`. CLI OAuth discovery now requests actual profiles and then paginated models at the selected region's management host; missing/ambiguous profiles fail without hardcoded ARN fallback. API-key directory discovery remains unsupported because no verified protocol is available. Existing canonical egress hosts are expanded only in an explicit revisioned draft prepare; custom policies are never silently expanded.

Ordinary live quota failures additionally distinguish unauthorized, forbidden, egress denial, invalid response and busy. Administrator login's explicit empty security requirement is retained; its ownership regression tests cover unauthenticated password login.

Protocol sources (implementation evidence, not upstream service guarantees):
- https://github.com/seakee/CPA-Manager-Plus/blob/main/apps/manager-server/internal/service/codexinspection/xai_probe.go
- https://github.com/chenyme/grok2api/blob/main/backend/internal/infra/provider/console/quota.go
- https://github.com/chenyme/grok2api/blob/main/backend/internal/infra/provider/web/quota.go
- https://github.com/QiXia881/xkiro.rs/blob/master/src/kiro/model/usage_limits.rs
- https://github.com/javargasm/opencode-kiro-auth/blob/main/src/models.ts

All claims above describe implementation. Actual account availability, private API/WAF compatibility and production acceptance remain separate gates.
