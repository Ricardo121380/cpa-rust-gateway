# P12-08 client migration inventory

Status: `IN_PROGRESS`

Date: 2026-08-02

This inventory is value-free. It records no endpoint, client key, model name, request/response body,
credential, source hash, or token fingerprint.

## Server-side usage inventory

- The incumbent CPA configuration contains three legacy client keys.
- CPAMP has observed three distinct key identities over its retained 30-day window. One has an
  operator alias and two are unaliased; none is identified by value in this report.
- During the latest one-hour window the incumbent handled no inference requests. The latest 24-hour
  inference records align with the bounded P12 differential/test window rather than an ongoing
  production workload.
- The retained 30-day history contains successful traffic across five model identities and four
  request paths, including Responses, Messages and Chat families. The latest seven-day window only
  contains Responses-family traffic from the current P12 work.

The absence of current traffic lowers cutover risk but does not prove that the old clients are
retired. Every configured legacy key must still be classified before replacement.

## Discoverable Mac mini clients

| Client | Evidence | Migration disposition |
|---|---|---|
| OpenClaw gateway | Active launchd service; its generated model registry contains one CPA Provider | [Isolated migration rehearsal](p12-08f3-client-migration-dry-run.md) passed with source unchanged. Issue a dedicated real CPAR key, restart, and run non-streaming + SSE smoke checks only in the cutover maintenance window |
| CC Switch / Claude | Historical inventory found one retained CPA Provider record | `DEFERRED_BY_OPERATOR`: do not read, copy, modify, restart, or test it in P12-08F3; migration or retirement requires a separate operator instruction before cutover |
| Codex through CC Switch | Historical inventory found no current CPA reference | Covered by the CC Switch exclusion; do not recheck or rehearse it in P12-08F3 |

Any client outside this Mac mini remains undiscovered. The three server key identities are the
authoritative upper bound until each is classified as migrated, deliberately retired, or owned by a
named external client.

## CPAR readiness gap

The active CPAR graph is still the bounded P12 differential graph: one upstream, endpoint,
credential, model, route, candidate, access group and client key. It is not yet a production catalog.
It must be replaced by an immutable production Config Version before the production hostname moves.

Historical Chat-family use is the compatibility gate. Release 1 deliberately leaves inbound Chat
Completions in P13. Therefore full CPA replacement requires one of two explicit outcomes before
P12-09:

1. every Chat client is confirmed retired or migrated to Responses/Messages; or
2. Chat compatibility is brought forward through a separately reviewed Change Request.

Silence during the latest seven-day window is not sufficient to choose the first outcome
automatically.

## Cutover checklist

- [ ] Classify all three legacy key identities without exposing their values.
- [ ] Confirm whether historical Chat clients are retired or require compatibility work.
- [ ] Create and publish an immutable CPAR production graph with the approved live provider set.
- [ ] Issue one CPAR key per retained client and record delivery/rollback ownership in a root-only
      operator ledger.
- [x] Rehearse the OpenClaw endpoint/key/alias transaction and byte-identical rollback in an isolated
      temporary configuration; confirm the live source remains unchanged.
- [ ] Issue OpenClaw's real dedicated CPAR key in the bounded cutover window; update, restart, and run
      non-streaming plus SSE smoke checks against the pre-exposure CPAR route.
- [ ] Resolve the operator-deferred CC Switch record through a separately approved migration or
      retirement action before production cutover.
- [ ] Freeze the incumbent CPA success/error/latency baseline and create byte-identical Caddy,
      client-config and service preimages.
- [ ] Validate the direct-cutover and rollback Caddy configurations on the live Caddy version.
- [ ] Only after every item above passes, begin P12-09 full cutover/rollback/recovery.
