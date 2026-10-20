# ADR-0116: Local Codex request metadata diagnostics

| Field | Value |
| --- | --- |
| Status | Accepted |
| Date | 2026-10-20 |
| Task / Matrix / Contract references | #62 C4 only; #58 T01/T02/T03/T08/T10; ADR-0107/0109/0115; [BC-PROTOCOL-010](../contracts/BC-PROTOCOL-010-local-codex-client-metadata.md) |

## Context

The user selected the Codex C4 subset of D1-A on 2026-10-20: CPAR becomes the final
recipient of individually reviewed client_metadata and retains it for authorized local
diagnostics. This is a new limited product decision. ADR-0107 concerns upstream response
metadata separation and did not authorize consuming request controls or correlation fields.
The frozen Codex snapshot contains execution, sandbox and approval declarations as well as
correlation identifiers. They must be classified, rather than assumed harmless from a string map.

## Decision

Accept only the field shapes and members in BC-PROTOCOL-010. CPAR provides local diagnostic
retention, without Codex server tracking, history ingestion, approval processing or native
upstream-consumption equivalence. Client declarations never become CPAR authority. The limited
path is authenticated standard Responses over HTTP, a route whose matching candidates all use
the actual GrokBuildResponses adapter, Responses target and Canonical/CanonicalBridge. An empty
or mixed/unqualified candidate set rejects. This conservative route scope prevents a locally
consumed field from later being routed to a different recipient. User-Agent, headers and generic
capability bits do not select this policy.

Before execution, the composition validates and removes exactly this field from the projected
execution request. The HTTP ingress confirms one Request event and its private companion in the
same existing finite queue slot and SQLite transaction. Only Persisted permits execution; a
queue-only/disabled result cannot acknowledge retention. The direct execution and Build encoder
boundaries continue to reject an unconsumed field. No plaintext enters model input, tracking
headers, GatewayEvent, telemetry, quarantine, public errors, audit or billing.

Use independent RequestMetadataEvidence, one per authenticated Request. Schema 37 adds a
request-keyed encrypted namespace and expiry receipts. Payload v1 of this new evidence is
independent of all existing v1 formats; no old payload, event or ledger shape is rewritten.
The payload repeats the exact Request event and the complete raw JSON field, including the
unchanged internal JSON string. Original admitted owner and canonical event indexes authenticate
reads. Multiple actual Attempts associate through their durable request_id; a Request with no
Attempt has none. Capture does not create Usage, ledger or Attempt events.

Reuse SecretStore/XChaCha20-Poly1305, external key ring, bounded writer, confirmation deadline,
private CLI output and maintenance owner. AAD separately binds owner, Request, payload version,
creation and exclusive expiry. Identical replay authenticates and compares the original without
renewing TTL; changed values, owner conflict, historical backfill and expired receipt replay fail.
GC removes ciphertext, retaining the non-payload receipt. Only the authorized read-only local
reader exposes plaintext in an exclusive mode-0600 file under a canonical mode-0700 directory.

## Consequences

Valid C4 metadata can accompany standard text/tools/known Usage without being sent to Build.
Pre-execution rejection after valid C4 admission can retain Request-only evidence; malformed C4
does not gain retention promises. A write/crypto/receipt failure prevents any upstream execution,
and no evidence failure triggers a resend. Request admission is stronger than terminal-only
confirmation: the evidence already exists before success bytes or any actual Attempt.

Namespace production activation remains BUILD_ACTIVATION_BLOCKED. ParallelTools, hosted search,
non-assistant input IDs, Claude metadata/controls/D2 and #61 cost/success metadata are independent.
No original-client or whole-ticket compatibility follows from field-isolated success.

## Alternatives considered

- Forward to Build: lacks target field semantics; the user selected local receipt instead.
- Treat this as ADR-0107 or generic metadata: conflates request responsibility and response delivery.
- Reuse Usage/summary evidence or add a storage platform: changes evidence meaning or duplicates
  the existing bounded encryption, confirmation and authorization mechanisms.

## Validation and rollback

Retain public C4 pre-rejection RED, then verify independent loopback receiver, actual public
listener/compiler/factory/leased Build path, JSON and fragmented SSE, exact private readback,
Request-only and actual-Attempt attribution, faults, replay/conflict, expiry/GC, keys and actual
process restart. Bind final checks and separate Standards/Spec reviews to immutable source and
enumerated overlays. Full does not imply original Codex or real Provider success.

Schema 36 -> 37 creates an empty namespace; old data stays UNKNOWN. New request readers accept
supported legacy 34–36 as UNKNOWN, schema37 as authenticated evidence, and reject newer schemas.
Existing Usage/summary readers remain compatible at schema37. Old binaries reject schema37;
binary-only rollback or down migration is not operational recovery. Preserve a coherent SQLite
snapshot, events/ledger/evidence/expiry receipts and every referenced external key, and validate
the selected reader on an isolated restored copy. This ADR authorizes no Provider call, remote
write, release, production restore or deployment.
