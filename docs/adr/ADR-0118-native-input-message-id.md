# ADR-0118: Native non-assistant input IDs with independent local qualification

| Field | Value |
| --- | --- |
| Status | Accepted |
| Date | 2026-10-21 |
| Task / Matrix / Contract references | #62 C6; #58 T02/T03/T08/T10; ADR-0115/0116/0117; [BC-PROTOCOL-012](../contracts/BC-PROTOCOL-012-native-input-message-id.md) |

## Context

Codex's frozen Message.id serializes on POST when present, but public non-assistant POST schema
and the fixed Build client do not qualify arbitrary caller-selected IDs at the fixed service.
The user explicitly authorizes an accurate bounded local contract and controlled implementation,
keeping production rejection. This is distinct from C4's approved local diagnostic recipient.

## Decision

Adopt the closed role/type/bounds/presence/conflict rules in BC-PROTOCOL-012. Preserve the original
string in existing message extensions and emit the same native id to the controlled Build
receiver. Reuse existing ID validation and cross-kind unique-history checks; validate the entire
effective history after stored continuation expansion. No dropping, hashing or local consumption.

Follow existing per-field namespace/search qualification seams: private gateway cfg(test)
composition plus the explicit dependency controlled-test-support feature. The actual Build
builder defaults to rejection. Runtime qualification independently checks adapter, Responses
source/target, Canonical/CanonicalBridge and HTTP before lease/send. Stock assembly, including
all-features, has no qualifying call. No header/config/URL/client identity confers qualification.
C6 grants none of the independent tool or parallel qualifications.

## Consequences

Native input IDs can survive JSON/SSE tool history and existing owned stored continuation in
controlled proof. They confer no routing, ownership, authorization, lookup or idempotency rights.
No Canonical or encrypted payload-v1 format, schema, reader, key, AAD or TTL changes are required.
Old readers may retain the envelope; old executors do not gain permission. Cross-request explicit
replay is legal; duplicate identity inside a single expanded history fails before execution.
Production BUILD_ACTIVATION_BLOCKED, actual Codex and #62 remain separately evaluated. Source
evidence and controlled receivers do not attest a serving profile or actual Provider behavior.

## Alternatives considered

- Delete/rewrite/hash the id: breaks the requested identity and content association.
- Consume through C4 diagnostics: changes the recipient and history semantics without authority.
- Infer POST permission from GET or client source: conflates representation and service support.
- Add an identity registry or rewrite persisted types: unnecessary for existing bounded history.

## Validation and rollback

Retain a real public-listener RED, then verify independent receiver values, order/content,
JSON/fragmented SSE, two tools, explicit and owned stored continuation, owner/lineage isolation,
invalid/duplicate/cross-kind IDs, independent qualification and ordinary paths. Bind affected
checks, required Full and independent Standards/Spec to immutable code/manifest/lock and exact
inherited overlays. Keep historical failures. Restore rejection to roll back execution; retain
all stored data and keys. No real Provider, remote write, merge, release or deployment is authorized.

Delivery evidence: [C6 bounded report](../reports/cpar-62-c6-input-message-id-20261021.md).
