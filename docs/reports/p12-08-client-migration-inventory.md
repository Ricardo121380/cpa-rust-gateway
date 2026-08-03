# P12-08 client migration inventory

Status: `P12_09_MIGRATED`

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

### 2026-08-03 live refresh

- OpenClaw's current live configuration is valid and no longer contains a CPA reference. It uses its
  existing non-CPA provider, so the production cutover requires no OpenClaw edit or restart. The
  earlier isolated rehearsal remains evidence for the transaction shape, not evidence that a live
  migration is still necessary.
- CC Switch remains excluded by operator instruction and was not opened or searched during this
  refresh.
- Legacy slot 1 matches the bounded P12 incumbent test identity and can be deliberately retired.
- Legacy slot 2 is unaliased and has been silent for approximately ten days. By elimination against
  the exact P12 test identity and the named Newapi identity, it is most likely the former OpenClaw
  key; current OpenClaw no longer references CPA. This remains an inference rather than a secret-level
  comparison because CC Switch is excluded.
- Legacy slot 3 is labelled `Newapi`, has been silent for approximately nine days, and the operator
  confirmed it must remain usable. It therefore requires an explicit CPAR key migration before
  cutover.

Slot 2 is a deliberate retired OpenClaw identity. Slot 3 was migrated to CPAR through Newapi's
official management API, restored to its complete CPA preimage during the mandatory rollback, and
migrated again after recovery. Both retained aliases passed JSON/SSE in the final state.

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

- [x] Classify all three legacy key identities without exposing their values: slot 1 is P12 test
      traffic; slot 2 is the high-confidence former OpenClaw identity and is retired; slot 3 is
      operator-confirmed Newapi and requires migration.
- [ ] Confirm whether historical Chat clients are retired or require compatibility work.
- [x] Create and publish an immutable CPAR production graph with the approved live provider set.
- [x] Issue one CPAR key per retained client and record delivery/rollback ownership in a root-only
      operator ledger.
- [x] Rehearse the OpenClaw endpoint/key/alias transaction and byte-identical rollback in an isolated
      temporary configuration; confirm the live source remains unchanged.
- [x] Recheck OpenClaw before cutover: its live configuration no longer references CPA, so no key,
      endpoint edit, restart, or smoke request is needed.
- [ ] Resolve the operator-deferred CC Switch record through a separately approved migration or
      retirement action before production cutover.
- [x] Freeze the incumbent CPA success/error/latency baseline and create byte-identical Caddy,
      client-config and service preimages.
- [ ] Validate the direct-cutover and rollback Caddy configurations on the live Caddy version.
- [ ] Only after every item above passes, begin P12-09 full cutover/rollback/recovery.
