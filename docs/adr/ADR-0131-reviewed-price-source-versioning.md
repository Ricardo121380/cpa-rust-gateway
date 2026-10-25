# ADR-0131 Reviewed price source and immutable application

- Status: Accepted
- Date: 2026-10-25 (Asia/Shanghai)
- Task: #73 / OPT-06; BC-MGMT-021
- Supplements: ADR-0078/0079/0080; preserves ADR-0128 frozen results

## Context

The existing public models.dev reader returns suggestions while immutable catalog imports already
own draft revision, transactional audit and forward-fork rollback. A browser must not turn a fresh
read into an unreviewed price change. Catalogs are global, so a selected draft revision alone misses
writes through a different draft. The existing catalog stores six explicit integer rates, with no
representation for absent rates. Replacing absence by zero would create false billing certainty.

## Decision

Reuse the source reader and existing catalog service. The protected preview records exact operator
mapping from source provider/model to Provider/Channel/public Model, source observation time and
SHA-256, units, nullable suggestions, explicit operator completion of absent rates, effective old
price version, and add/change/unchanged/omitted differences. Parse original JSON decimal lexemes
with checked integer scaling; reject precision loss, unknown tier keys, units and currency conflicts.
Unknown models and incomplete rates remain visible and block application. No fuzzy matching,
unknown-tier flattening, source-value override or implicit zero is permitted.

A preview is an actor/config/revision-bound process-local handle, ten-minute TTL, at most eight
handles and 512 KiB reviewed content each. Confirmation supplies its content digest, never replacement
prices. It rereads the same fixed public source and compares raw-content SHA-256. Inside the existing
immediate write transaction it also compares a canonical fingerprint of all immutable catalog rows.
This detects imports/rollbacks through other drafts. Revision bump, catalog, retained review evidence
and billing_catalog_review_applied audit commit together. Duplicate confirmation conflicts; failures
keep the handle available until expiry for a safe retry when the exact inputs are still valid.

Effective time is explicit and must remain at or after confirmation observation time. Prices apply
to new materialization using exact Provider/Channel/public Model and Attempt end time. Equal-time
catalogs follow the existing lexical-ID tie break; a candidate already shadowed at its effective time
is blocked. An omitted tuple is removed only from the proposal: the existing sparse catalog lookup
continues selecting its retained prior price. This effect is displayed. Routing-price policy changes
still require their existing separate mutation and configuration application.

Migration 0040 adds only catalog review evidence, keyed to immutable catalog identity. Existing
catalogs/ledger rows are retained byte for byte; legacy imports have no review evidence and GET
returns 404 for them. Retained evidence includes the reviewed mappings and both source and content
digests, but never raw source documents, credentials, request content or configurable source URLs.
Rollback remains the existing audited new-version fork; the original review can be queried by its
original catalog identity. Historical ledger and already captured #72 results are never repriced.

## Consequences

No optional-rate schema or pricing-algorithm migration is needed: operators explicitly complete
missing dimensions in the proposal or retain an unresolved preview. This is more work for a source
with only input/output rates, but preserves the existing billing meaning. Unrecognized unit/tier
rules remain blocked even if an operator supplies rates. Restart invalidates pending handles; applied
review provenance survives. Whole-document source hashes conservatively invalidate a preview on
unrelated source changes or formatting changes. A source cannot offer atomicity with our database;
confirmation proves equality of the last completed read, not continued remote immutability.

## Alternatives considered

Direct import would not bind the reviewed source or catch cross-draft changes. Stateless client
submission could change the reviewed content. Adding nullable rates throughout pricing/router/store
would widen the scope and change existing billing semantics; silently zero-filling is rejected.

## Validation and rollback

Controlled real loopback management HTTP covers source failures, unknown/missing/invalid prices,
CSRF, digest/config/revision/global-source fences, audit failure atomicity, duplicate confirmation,
review persistence and forward rollback. The #72 real HTTP fixture compares all CSV/JSONL bytes
for both grains across application and new materialization, plus old/new ledger identities.
Operational rollback uses a new catalog version; downgrade 40 to39 drops only review provenance,
retaining catalogs/ledger. Pending handles require a new preview. No real Provider or paid source.

Number 0131 is reserved in the parallel audit directory; 0130 is left for parallel #69. Integration
must reconcile later reservations instead of overwriting them.
