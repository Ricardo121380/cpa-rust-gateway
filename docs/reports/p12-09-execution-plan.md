# P12-09 production cutover execution plan

Status: `IN_PROGRESS_P1_FIX_BUILD`

Date: 2026-08-03

This report is value-free. It contains no endpoint value, model name, credential, client-key value,
request or response body, token fingerprint, or mutable resource ID.

## Authorized outcome

The operator approved starting P12-09. The only accepted production topology is direct replacement:
the production hostname points entirely to CPA or entirely to CPAR. No percentage, header, key, or
fallback split is allowed. P12-09 must perform one CPA to CPAR cutover, one complete rollback to CPA,
and one recovery to CPAR. P12-10 owns the subsequent 72-hour observation and retirement of CPA.

## Completed pre-cutover work

- GitHub CI run `30768254180` passed Classify, Fast, Full supply-chain, and Required delivery gates for
  commit `7132d09`; this closes the P12-08 delivery dependency.
- After the P1 safety rollback, the server again routes the production hostname only to CPA. CPAR is active, disabled at boot, and
  listens only on the two reviewed loopback ports. The public Caddy configuration has no management
  listener route.
- A root-only backup set was created at
  `/var/backups/cpa-rust-gateway/p12-09-20260803T011044Z`. It contains the live Caddy preimage, a
  validated direct-cutover candidate, a byte-identical rollback candidate, an online SQLite backup,
  CPAR credential archive, incumbent state archive, unit preimage, hashes, and a value-free manifest.
  The control database backup passed `PRAGMA quick_check`.
- Both complete Caddy candidates passed the live Caddy validator. The cutover candidate removes the
  temporary staging hostname, changes exactly the production site to CPAR, and contains no public
  management route. No reload has occurred.
- CPAR loopback preflight passed Chat Completions, Responses, and Messages in both JSON and SSE modes.
  The four P1 observability counters for required quarantine, required write failure, required queue
  full, and sink closed did not increase.
- OpenClaw's live JSON is valid and contains no CPA reference; its only configured model provider is
  unchanged. No OpenClaw edit or restart is required. CC Switch remains excluded from inspection,
  modification, restart, and migration rehearsal by operator instruction.

## Client disposition gate

The incumbent still has three configured legacy key slots. Their values and hashes were never
printed or copied:

| Slot | Evidence | Proposed disposition |
|---|---|---|
| 1 | Matches the bounded P12 incumbent test identity; last used in the P12 window | Retire at cutover |
| 2 | Unaliased; no use for approximately ten days. After excluding the exact P12 identity and the named Newapi identity, the only independently discovered historical CPA client is the former OpenClaw provider; current OpenClaw no longer references CPA | High-confidence retired OpenClaw identity; retain rollback evidence, do not migrate unless contrary ownership evidence appears |
| 3 | Alias is `Newapi`; no use for approximately nine days | `MIGRATION_REQUIRED` by operator instruction; issue or assign one dedicated CPAR key and update the actual Newapi deployment before cutover |

Slot 2 is an evidence-backed inference rather than a secret-level equality proof because CC Switch
remains excluded and no token fingerprint may be compared or retained. The production Caddy reload is
now prohibited until slot 3's actual Newapi deployment location is identified, its CPAR key is
delivered without entering chat or Git, and a pre-cutover authenticated smoke check passes.

### Jakarta Newapi access resolution

The private key was valid. Clash TUN intercepted the direct SSH path and produced the misleading
identity/authentication symptoms. After backing up and correcting the narrow TUN exclusion, the
existing strict SSH alias connected successfully without replacing the key or weakening host checks.
Newapi was then backed up and updated through its official management API, not direct SQL mutation.
Its two existing public model names stayed unchanged, its ability cache was regenerated, and both
aliases passed Messages JSON and SSE. The P1 safety rollback restored the complete Newapi channel
preimage through the same official API.

## Executed transaction and P1 stop

- The original health-body RTO classifier was invalid because CPA and CPAR expose the same fixed
  health body. The helper now supports an authenticated status classifier: the successor-only key
  returns `200` at CPAR and `401/403` at CPA without recording a credential or response body.
- Valid effective Caddy measurements were 89ms for the initial cutover, 89ms for the required
  rollback, 88ms for recovery to CPAR, and 89ms for the final P1 safety rollback. The earlier
  health-body receipt is explicitly invalid and is not counted.
- The public production entry passed authenticated Models, unauthenticated rejection, Chat,
  Responses and Messages in JSON and SSE modes, plus Chat Tool. Newapi also passed both retained
  aliases in JSON and SSE modes. Database quick checks passed and management/data listeners remained
  loopback-only.
- The final observability check found `required_quarantined` had increased from zero to two while
  write failure, required queue full and sink closed stayed zero. The two missing durable events were
  final Usage records for otherwise successful Chat SSE requests. This is a P1 signal, so the
  transaction immediately returned Caddy and Newapi to CPA and did not start P12-10.
- A bounded diagnostic while production remained on CPA proved the upstream reused an eight-byte
  response ID already present in the Usage log; the request succeeded but the quarantine counter
  increased once more. No identifier value or response content was retained.
- The local fix makes final Usage identity request-scoped with a domain-separated SHA-256 digest of
  the gateway-owned request ID. It still validates the externally supplied response ID length.
  Focused tests prove reused upstream IDs persist for distinct requests, identical replay remains
  idempotent, conflicting same-request replay still quarantines, and oversized identifiers remain
  bounded. Exact-revision artifact build, deployment and a fresh zero-delta production transaction
  are still required.

## Cutover transaction after the gate clears

1. Freeze a fresh CPA request/error/latency baseline and the four CPAR P1 counters.
2. Run the RTO helper with the complete cutover Caddyfile and verify that the production health probe
   observes CPAR rather than merely trusting a successful reload return.
3. Verify production TLS, unauthenticated rejection, the retained CPAR key, all three JSON/SSE
   protocols, Tool semantics, Usage, Route Explain, Attempts, database integrity, and P1 counters.
4. Install the byte-identical rollback Caddyfile and measure effective routing recovery to CPA.
   Verify the legacy test key and record the client-authentication recovery interval.
5. Reinstall the reviewed cutover Caddyfile, measure recovery to CPAR, repeat the bounded consistency
   checks, and begin the separately receipted P12-10 72-hour window.

Any P0/P1 signal, authentication mismatch, semantic regression, unexplained route, database failure,
or failed effective-route observation stops the transaction at CPA and blocks P12-10.
