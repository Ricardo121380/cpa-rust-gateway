# ADR-0133: Explicit production strategies and ordinary Build cache affinity

- Status: Accepted
- Date: 2026-10-25 (Asia/Shanghai)
- Task / Contract: #70 / OPT-03; [BC-ROUTE-010](../contracts/BC-ROUTE-010-production-strategies-build-affinity.md)
- Limited successor: ADR-0045, ADR-0084/0085/0086. ADR-0087 pricing and ADR-0091/0092 exact lineage remain.

## Context

The common baseline is `7c16070bb66f7f3008022f7870b5d1f96b63f167`, including #69/#73/#75 and schema40.
The serving Provider selector ranked quota evidence, explicit price evidence, load, ascending
priority, descending weight and stable identities. Its weight tie breaker was not traffic share.
The separate route schedule and endpoint credential pool already owned cursors and leases, but
ordinary single-Provider serving bypassed the former. Build derived a cache identity after account
selection; its durable affinity interface had not been connected to serving.

CPA is fixed at v8.0.40 / `d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683`:
`sdk/cliproxy/auth/selector.go::{RoundRobinSelector.Pick, WeightedRoundRobinSelector.Pick,
FillFirstSelector.Pick, availableAuthsFromPriorityBuckets, pickSmoothWeightedAuth}` and
`sdk/cliproxy/service_config.go` are the source comparison. CPA uses higher numeric priorities;
CPAR retains lower numeric priorities. This is a behavioral adaptation using CPAR's existing
bounded compiled schedules, not a port of CPA's mutable accumulator or its larger weight limits.
No upstream source or fixture is copied.

A later concurrent-state check found #71 independently uses migration0041/schema41 for endpoint
request policy. These standalone schema41 identities differ. Do not move either schema41 database
into the other binary. Shared integration must order and reconcile both migrations, retaining the
common schema40 prefix; this ticket neither imports #71 nor claims an integrated schema41.
The shared maintenance COORDINATION.md records the collision and unique ADR0133/0134 ownership.

## Decision

The protected Route `policy` explicitly admits `round_robin`, `weighted_round_robin`, and
`fill_first`. The existing `smooth_weighted_round_robin` continues to mean the prior production
Provider-scoped comparator; existing `priority_failover` rows remain readable but unsupported by
production mutation/composition. No existing value is silently relabelled or priority reversed.
CPA `round-robin`, `weighted-round-robin`, and `fill-first` map to these three snake-case values;
an operator must choose the value explicitly. There is no automatic CPA configuration importer.

For explicit policies, hard eligibility comes first. Candidates are grouped by ascending priority,
quota evidence, and separately enabled routing-price evidence. A strategy acts only inside an
identical group. RR gives every candidate one slot, WRR retains each candidate's configured slot
weight, and fill-first always starts at stable candidate ID order. The endpoint-local credential
schedule is a second independent decision, with its own ascending binding priority and credential
weights; candidate weight is never multiplied by the number of accounts. RR ignores credential
weights, WRR uses them, and fill-first uses stable credential ID. Temporary exclusions filter slots
before cursor advancement rather than awarding a rejected candidate's slots to its successor.
Explicit candidate weights are 1..=10,000 at snapshot compilation, including retained/imported rows; the existing 1024-slot tier bound also applies to weighted schedules. Invalid candidates cannot replace a serving generation. Publication/catalog replacement resets candidate cursors; an identical catalog snapshot retains
them. Endpoint cursors belong to that composed pool, independently for legacy, RR and explicit WRR so mixed policies do not move one another. Shared concurrency survives runtime generations.

Ordinary Build requests with `prompt_cache_key` use the existing HMAC identity and continuity store.
Read candidate-scoped preferences once at request startup on a blocking task; the scheduler receives
only immutable request-local hints. It never queries SQLite or sends a Provider request. An exact
revision hint may win only inside the applicable candidate confidence/priority group and endpoint
credential priority, after fresh protocol, owner, catalog, Health, Quota, expiry and atomic capacity
checks. A failed hint uses the normal strategy and existing retry loop.

A successful upstream bootstrap writes the actual account/revision and the selected transport's
opaque egress identity. The binding fingerprint includes Config Version/revision, candidate,
endpoint, Provider, canonical public model, upstream model, credential revision and egress. A new
configuration or changed transport cannot consume an older namespace. This is soft cache reuse,
not a guarantee that the Provider retained a cache or that an entire response completed. Strict
stored/compact/reasoning/WebSocket pins and diagnostic channel pins never attach soft affinity.

Bindings have a 15-minute sliding TTL and a global 4096-row capacity. Successful writes remove
expired rows and evict oldest updates (stable rowid tie break); absence after expiry/eviction is an
ordinary strategy miss. Break history retains its latest 4096 rows. Every changed credential,
egress, revision or namespace commits together with a break. Expiry and egress changes are exact
closed reasons; `policy_reselection` deliberately does not invent a root cause for capacity,
revision/configuration mismatch or a concurrent winner. Token loss is NULL when unmeasured.
Concurrent misses can select independently; the last successful bootstrap transaction determines
the next hint. A stale-generation write can never qualify a current-generation hint. Store failures
fail the affected request safely; a persistence failure after bootstrap does not authorize a retry.

Migration0041 adds a version-scoped selection encoding beside the unchanged legacy policy column,
nullable affinity qualification, and nullable break loss. Schema40 affinity rows remain unqualified
for serving; their old values and known loss records survive migration. Down-migration rejects
explicit new policies, qualified bindings, new break reasons or UNKNOWN loss instead of discarding
information. Old ledger, immutable catalogs/reviews, frozen exports and encrypted request payloads
are untouched. Importing/applying a price remains separate from binding a routing price policy.

## Consequences

Route CRUD, validation, publication/rollback and Explain share the actual policy. Explain includes
`selection_policy`, projects fixed input and does not reserve capacity or advance live cursors;
it is not a promise of the next request under concurrency. Native Build model-alias qualification
remains independent: synthetic receiver aliases in tests do not activate real service aliases.
The old deterministic default and exact continuation contracts remain useful and unchanged.

## Alternatives considered

Changing the legacy comparator or repurposing its weight tie breaker would silently alter existing
configurations. Replacing the account pool would split lease ownership. Encoding raw identities or
proxy URLs in affinity rows would expose private material. Inferring token loss as zero would
manufacture evidence. A universal Provider session framework and #71 request/header rules are not
needed for this seam.

## Validation and rollback

Public TCP-to-independent-receiver tests exercise independent-identity distribution, repeated-owner
reuse, expiry, owner isolation, saturation/concurrency, revision/Health changes, retries, semantic
closure and strict stored lineage. Control and management tests exercise policy persistence,
validation/publication/rollback and closed inputs; capacity/migration tests exercise schema40 and
NULL loss. Full, explicit backup checks, review findings and frozen source/binary manifests are
recorded in the [issue70 implementation report](../reports/issue-70-routing-build-affinity-20261025.md). Real Providers/clients, fixed-service qualification,
remote CI, release, deployment and cost admission remain separate.
