# ADR-0128: Account quota observation history

| Field | Value |
|---|---|
| Status | Accepted |
| Date | 2026-10-25 |
| Task / Contract | [#74](https://github.com/Ricardo121380/cpa-rust-gateway/issues/74), OPT-07 / [BC-MGMT-018](../contracts/BC-MGMT-018-account-quota-history.md) |

## Context

Existing management reads project account quota into memory, while the Kimi catalog worker
also observes quota in its existing pass. Operators cannot inspect past windows or gaps after
restart. Serving quota tables are latest-state routing inputs and are inappropriate history owners.

CPA reference: v8.0.40, `d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683`,
`sdk/cliproxy/auth/types.go::QuotaState` and
`internal/api/handlers/management/auth_files.go::quotaObservationPayload` distinguish passive
snapshots from cooldown state and from accumulated consumption. This implementation follows
that boundary; the new SQLite history/query is CPAR-specific, not a port of CPA plugin polling.
No upstream code or fixtures are copied.

## Decision

- Add independent append-only `quota_history` schema 39. Keep internal account kind, ordinary
  configuration scope, account ID and revision, channel, source, observation time and projected
  window evidence. Account deletion preserves prior samples, without cascading into history.
- Capture a fence before the existing read; append in an immediate transaction that rechecks
  revision, encrypted-material identity and lifecycle state. Ordinary runtime captures must also
  match the current pool material revision; caches are revision-bound. The digest is memory-only. No account, routing,
  current-quota, Request/Attempt/Usage or billing rows are mutated by history.
- Retain every completed read with a distinct observation timestamp. Cache hits preserve their
  timestamp and coalesce. Same-time equal samples coalesce, conflicting samples reject the entire
  batch; older samples of the same revision/series are ignored. Source/window keys stay separate.
- Reuse ordinary Codex/Claude/Kiro management reads, native Build/Console/Web usage reads, and
  existing Kimi management/background metadata. There is no new worker, frequency, inference
  probe, account enable, reset or recovery action. Existing passive router 429/header state and
  currently unwired native Quota worker are not silently converted into account history.
- Confidence is `observed` or `unknown`. Completeness is `projected` (recognized fields only),
  `partial`, or `unknown`; none claims an entire account balance. Status distinguishes observed,
  explicitly exhausted, unknown and failed. Freshness is an independent query-time flag, bounded
  by five minutes and any explicit reset. This freshness ceiling does not promise a polling interval.
- Only numerical quota fields and structural window IDs persist. No email, plan, raw response,
  headers, secret, or free-form failure/reset/unit text is retained. RFC3339 resets become epoch
  milliseconds; unparseable resets remain unknown. Free-form Build product labels become stable
  SHA-256 window identifiers. Kiro positional windows remain partial and never claim reset changes.
- `GET /admin/quota-history` requires explicit time bounds (at most 31 days) and a page size
  of 1–200. Optional account kind, configuration, account, provider and window filters use bound
  parameters. Append-ID pagination is live, not a frozen export snapshot; keep filters unchanged.
  A window filter includes windowless failures/unknowns. The existing management auth and blocking
  read concurrency bound apply. No upstream call is made by this endpoint.
- A missing coverage interval and changed reported reset timestamp are evidence annotations,
  never proof of actual consumption, recharge, entitlement or billing. Missing time after a sample's
  validity ceiling remains unknown. No interpolation, subtraction or rollup is performed.

## Consequences

Quota, token Usage and cost retain separate owners and interfaces. History survives restart and
account deletion. There is no inferred pre-upgrade history or automatic retention/deletion policy.
A storage failure cannot be reported as a successfully recorded observation. Unsupported readers
may still return the existing explicit unsupported result without producing a history sample.

## Alternatives considered

Replacing serving quota rows loses observations and changes scheduler behavior. Persisting raw
provider responses or identity would retain unnecessary private material. Periodic new polling
would expand network activity and requires its own explicit configuration/authorization.

## Validation and rollback

Management HTTP tests cover filtering, duplicate/conflict/older samples, unknown/failure,
partial/exhausted/stale states, gaps, reset reports, account mutation and retained history.
The stock serve acceptance uses a controlled TLS peer, dynamic ports, child-only DNS/connect
interposition and an OS outbound fence; background tasks are disabled. It terminates and starts
a new process with the same database and checks readback plus zero Usage/ledger rows.

Migration 0039 is additive; its down migration drops only history. Preserve a backup before any
explicit downgrade that would discard history. Schema/ADR numbers are local to this branch and
must be reconciled with concurrent #68/#72 branches before integration. No integration, remote CI,
actual client, real Provider, release or deployment evidence follows from local tests.
