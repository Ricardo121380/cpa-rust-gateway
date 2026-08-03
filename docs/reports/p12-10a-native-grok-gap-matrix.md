# P12-10A native Grok gap matrix

Status: `DONE`

Reference: grok2api tag `v3.0.10`, commit
[`c27f0545197b3edf41d5deedcc2c3c3597887766`](https://github.com/chenyme/grok2api/tree/c27f0545197b3edf41d5deedcc2c3c3597887766).

The reference was checked out read-only outside the CPAR workspace. No code was copied into the
repository and no grok2api database or account was changed.

## Mapping

| Concern | Frozen grok2api behavior/source | Existing CPAR destination | Gap / port decision |
|---|---|---|---|
| Provider identity | `domain/account/account.go`: Build, Web and Console are separate providers; links do not share health/quota | `provider-grok` separates Build, Web and Official; Web primitives exist | add Console identity/runtime; preserve independent binding state and explicit links |
| Credential aggregate | `Credential` stores auth type, encrypted access/refresh/cookie, expiry/refresh state, priority, concurrency, cooldown and reauth | versioned encrypted `upstream_credentials`; mutable Build credential runtime | add provider-neutral native Grok account metadata and mutable revisions without making Config Version hot-mutable |
| Pool selection | `application/gateway/selector.go`: eligible candidates, bounded per-account concurrency, priority, cooldown, quota/model blocks and recovery | `RouteCredentialScheduler`, `EndpointCredentialPools`, `RuntimeHealthRegistry`, `RuntimeQuotaRegistry` | reuse CPAR scheduler; each account maps to a credential binding; do not port the Go selector wholesale |
| Large-pool reads | layered account bases/model overlays, short cache and singleflight with authoritative fallback | immutable route snapshot plus sharded runtime registries | build pool snapshots on control-path changes; no SQLite query or cache refresh in inference hot path |
| Refresh | `CredentialRefreshDueAt`: deterministic 5-8 minute pre-expiry jitter; bounded batch refresh and persistent failure/reauth | `GrokBuildCredentialRefreshCoordinator`: per-key singleflight, CAS-sealed revision | add proactive due queue, bounded workers, restart recovery and durable backoff around existing CAS refresh |
| Quota | account windows, model blocks, free/paid recovery and controlled probes | binding/model quota scopes and recovery tickets already exist | import/sync into existing quota registry and durable Grok runtime tables; keep exact account/model scope |
| Failure | exponential cooldown; Console/Web 403 may mean egress clearance, not invalid credential | exact credential/model health, forbidden recovery and Web failure attribution | extend Console classification; only independent account evidence may mark reauth/forbidden |
| Console import | `infra/provider/console/import.go`: SSO account import with provider-specific normalization | no Console credential importer | implement strict, bounded Console seed parser and sealed CPAR account import |
| Console inference | `infra/provider/console/adapter.go`: stateless Responses target, SSO bearer, SSE/JSON, rate-limit metadata, egress clearance | unified Canonical runtime, DNS-pinned transport, Chat/Responses/Messages bridge | implement `grok.console.responses`; decode to Canonical once, then reuse three-protocol output projection |
| Control operations | batch import, refresh, quota sync, reauth cleanup | protected management API, audit and backup/restore | add bounded batch jobs and value-free progress/receipts; no browser/admin secret exposure |
| Migration | grok2api owns a different encryption envelope/key | CPAR master-key credential envelope | source decrypt -> pipe -> CPAR validate/re-encrypt; never copy ciphertext or write plaintext temp files |

## Locked invariants

1. grok2api is never in the final CPAR request path.
2. Build/Web/Console account health and quota remain isolated even when identities are linked.
3. inference selection does not query SQLite, refresh OAuth, synchronize quota or mutate the routing
   graph; it only acquires an existing bounded lease and reports classified outcomes.
4. a refresh result commits by revision CAS; a stale worker cannot overwrite a newer credential.
5. no request failure disables an account globally unless the failure contract supplies exact
   account-level evidence.
6. migration is idempotent by provider-specific stable identity, transactional by bounded batch and
   fully reversible before grok2api retirement.
7. public Chat, Responses and Messages are projections over one native Provider execution, not
   three unrelated account pools.

## Review result

The smallest correct implementation reuses CPAR's existing credential pool, runtime health/quota,
encrypted credential and Canonical protocol layers. P12-10B should therefore begin with the native
Grok account aggregate/import contract and schema, not with another scheduler or public endpoint.
