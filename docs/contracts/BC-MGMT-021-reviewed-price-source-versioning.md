# BC-MGMT-021: Price source preview and confirmed version application

- Status: Accepted; local validation recorded separately
- Task: #73 / OPT-06
- Decision: [ADR-0131](../adr/ADR-0131-reviewed-price-source-versioning.md)

## Requests and outcomes

`POST /admin/billing/price-source/refresh` retains the fixed public models.dev source and returns
source_digest/observed_at_ms, USD and microunits-per-million units. Exact model matches retain
source provider identity; unmatched requests have unknown_model issues. Missing values are null.
No credential, browser-supplied URL, model body or internal identifier is sent to the source.

`POST /admin/billing/price-source/preview` requires management authentication, applicable Origin/CSRF,
X-Config-Version and exact If-Match. Input is catalog_version_id, effective_at_ms and 1–512 unique
mappings. Each mapping contains source_provider/source_model and exact provider_id/channel_id/model.
Optional missing_rates fields (input/output/reasoning/cache_read/cache_creation/cached) are explicit
operator completions in integer microunits per million; they cannot contradict source prices.
Only complete six-rate proposals without issues receive can_apply=true, preview_id/content_digest
and expires_at_ms. Missing/unknown rates are never filled with zero. Source prices are exact scaled
JSON numbers; excess precision, unknown pricing/tier keys, currency/unit conflicts or shadowing at
the requested effective time block application. The source contract is USD per million tokens.

The response lists old effective catalog ID and rates, source rates, proposed rates, explicit mappings,
add/change/unchanged differences, and omitted tuples in selected channels. Omission is explicitly
removed_from_proposal/retains_prior_price: global sparse price selection still uses the prior catalog.
No known price is deleted by omission. Effective time must be future/current at preview and confirmation;
selection uses Attempt end time, not request start or materialization wall clock. Catalogs at the same
effective time use the existing lexicographically greatest ID. A future higher catalog can supersede
this version later. Existing routing-price policy is not changed or activated by price application.

`POST /admin/billing/price-source/apply` accepts only preview_id/content_digest with the same selected
Config Version/If-Match and actor. It rereads source content, then compares all global catalog content
inside the write transaction. Source or catalog changes (including another draft), expired/missing/
consumed handle, wrong digest/config/revision or elapsed effective time require another preview (409).
Source failure is502; admission busy is429. Success201 creates one immutable imported catalog and
one billing_catalog_review_applied audit event, with one revision increment. Catalog/review/audit
failure rolls everything back. Failed reads/writes do not consume the handle; identical retry remains
possible until expiry if source/catalog/revision still match. Duplicate success is409, with no extra audit.

`GET /admin/billing/catalogs/{id}/review` reads durable safe review evidence for the selected configuration
context, including source and content digests and original reviewed differences. Legacy imports and
rollback forks without a review return404; follow the rollback receipt's rolled_back_from to the retained
predecessor. Existing rollback creates another immutable operator version and audits it. Catalog capacity
remains256. All responses use the existing safe error envelope and no-store management boundary.

At most eight pending handles per process, ten-minute TTL and512 KiB each. Restart invalidates pending
handles; confirmed evidence is durable in schema40. No raw source response or access-bearing material
enters public errors/audit/review. Preview is read-only with respect to durable configuration.

## Historical invariant

No endpoint rewrites ledger, runs materialization or edits captured usage results. The old ledger
catalog ID and amount remain unchanged. New materialization selects its effective version; missing
catalog and unknown Usage remain unpriced/unknown, never zero. Frozen #72 metadata, details, buckets
and all CSV/JSONL bytes remain unchanged across application and later Usage. Historical repricing and
complex tier semantics require explicit independent scope.

## Corresponding tests

- `source_decimal_rates_never_round_or_guess_tiers`
- `price_preview_http_fences_sources_revisions_replays_and_audit_atomically`
- `usage_snapshot_http_freezes_totals_details_and_exports`
