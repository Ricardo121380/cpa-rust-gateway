# #73 price source preview/version application — local implementation

Date: 2026-10-25 Asia/Shanghai. Validation in progress; no PASS claim until the final evidence is recorded.

Baseline: `84fbfbf778b582f746c1cafe31a2ce58962e8f77` from the verified #68/#72/#74 integration.
Branch `feature/73-price-preview-versioning`, independent worktree
`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar-73-price-preview-versioning-20261025`.

Scope and semantics: [ADR-0131](../adr/ADR-0131-reviewed-price-source-versioning.md),
[BC-MGMT-021](../contracts/BC-MGMT-021-reviewed-price-source-versioning.md).

Fixed CPA reference: v8.0.40 / `d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683`.
The official tag and complete tree were checked; 97 management/redisqueue/SDK Usage Go files and both
management route files in the local pinned source copy matched their Git blob identities. Actual
comparison symbols: `GetUsageQueue`, `parseUsageQueueCount` in
`internal/api/handlers/management/usage.go`; `GetAPIKeyUsage` in `api_key_usage.go`;
`usage.PublishRecord`/`Plugin.HandleUsage`, `redisqueue.Enqueue`/`PopOldest` and
`TokenAccountingSchemaVersion=2`. The queue read consumes records. Within those verified sources
and management routes there is no price source/catalog-version/price-review service or
`ImportUsage`/`ExportUsage` symbol. This is a bounded comparison, not a full-repository negative claim.
This price workflow extends existing CPAR services rather than claiming a CPA implementation port.

No actual Provider, paid source, real credentials, remote write, push, merge, deployment or ticket
closure is authorized or performed. Test sources are injected synthetic public-format documents.
Pending preview state is ephemeral; applied review evidence is durable schema40. Existing six-rate
catalogs remain complete: missing rates stay null/blocked until explicitly completed by operator.
No historical repricing, complex tiers, optional-rate billing schema or routing-policy activation.
