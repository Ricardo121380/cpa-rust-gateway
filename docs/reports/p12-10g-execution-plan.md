# P12-10G controlled live migration and parity plan

Status: `DONE`

## Objective

Prove that a bounded real grok2api subset can move through the P12-10F memory pipe into an
isolated CPAR database, remain attributable to its exact native account, execute through CPAR's
native provider boundary, preserve the three public protocol projections, and roll back without
changing or deleting the grok2api source pool.

This is not a production cutover. P12-10H still owns the grok2api stop/start drill, representative
72-hour CPAR-only observation and any permanent retirement.

## Read-only preflight

On 2026-08-04 the current server reported both `cpa-rust-gateway.service` and the pinned
`grok2api:v3.0.10` container active. CPAR remained loopback-only on its established data and
management listeners. The source database independently reported:

| Provider | Source rows | Enabled | Active | Authoritative expiry rows | Controlled disposition |
|---|---:|---:|---:|---:|---|
| Build | 829 | 829 | 1 | 829 | one active, unexpired account may enter the first isolated batch |
| Console | 898 | 898 | 898 | 0 | one active account may enter a separate isolated batch; Console SSO has no expiry field requirement |
| Web | 898 | 898 | 898 | 0 | reject before export because CPAR requires evidence-backed absolute Web session expiry |

The root-only helper then compared the independent SQLite count, the export response count header
and the decoded export array length. Build produced one bounded record from 829 source rows,
Console produced one from 898, and Web failed closed as `web_expiry_unavailable` before login or
credential export. During this preflight successful plaintext output went directly to `/dev/null`;
no account material was written to a file or returned in the receipt.

## Execution boundary

1. Build an exact Linux gateway artifact containing the reviewed local `grok-import` and
   `grok-rollback` commands. The commands require effective uid 0, an absolute direct database,
   the existing direct systemd credential directory, a bounded batch identifier and a non-terminal
   stdin pipe. Their output contains only fixed categories and counts.
2. Create a root-owned temporary directory on the server, copy the current CPAR database into it,
   and run SQLite integrity checks. Do not open the production database with the new command.
3. Stream exactly one active Build export into one isolated import batch. Independently verify the
   source/export/emitted counts, the CPAR created count, schema integrity and absence of source
   identity or token text in operator output. Repeat under a separate batch for one Console account.
4. Compile and exercise only those imported native accounts through a dedicated CPAR live harness.
   Each request must retain a single account attribution, bounded quota/cooldown state and one
   terminal Canonical sequence. The same semantic result must remain projectable as Chat
   Completions, Responses and Messages. No generic retry or fallback to grok2api is allowed.
5. On the first failed invariant, stop the isolated harness and roll back every applied batch.
   On success, perform the same rollback deliberately. Prove zero live native accounts remain in
   the isolated CPAR copy and that the source counts, source service health and production CPAR
   database are unchanged.

## Explicit exclusions

- no production graph publication, Caddy/Newapi/CC Switch change, public listener or client move;
- no source account update, refresh, quota mutation, disable, delete or grok2api restart;
- no Web import until an absolute session expiry is supported by controlled evidence;
- no plaintext credential file, shell argument, environment variable, log, report or receipt;
- no P12-10H stop/start or 72-hour observation.

## Acceptance and rollback

P12-10G can be marked `DONE` only after the isolated live batches, exact account attribution,
quota/cooldown checks, direct provider attempts, three-protocol parity and deliberate rollback all
pass. A valid source export plus import alone is insufficient. Any missing runtime evidence leaves
P12-10G open and production unchanged.

## Result

The controlled live execution passed on 2026-08-04. Source-database count, export-response count
and decoded export-array length agreed for the complete Build and Console pools. One eligible
account from each provider then passed the isolated native CPAR import, direct provider attempt,
exact account attribution, available health/quota projection, complete Canonical lifecycle and
Chat Completions/Responses/Messages projections. Web stopped before export as
`web_expiry_unavailable` because no authoritative absolute session expiry was available.

Both accepted batches were deliberately rolled back. The isolated database finished with zero
native Grok accounts and `quick_check=ok`; the production database fingerprint, production CPAR
service and grok2api health were unchanged. The exact signed gateway artifact and the reviewed
export helper revisions are recorded in the
[P12-10G live receipt](evidence/p12-10g-live-subset-receipt-20260804.md). P12-10H has not started.
