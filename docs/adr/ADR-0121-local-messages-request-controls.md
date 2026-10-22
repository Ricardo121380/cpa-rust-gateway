# ADR-0121: Local Messages keep-all and private user metadata

| Field | Value |
| --- | --- |
| Status | Accepted |
| Date | 2026-10-22 |
| Task / Matrix / Contract references | #64 M3/M4 only; #58 T01/T02/T03/T08/T10; ADR-0100/0114/0116/0120; [BC-PROTOCOL-015](../contracts/BC-PROTOCOL-015-local-messages-request-controls.md), BC-ROUTER-004, BC-OBS-007 |

## Context

The user explicitly selected D1-A's Claude M4 subset independently of Codex C4. CPAR is the
final local diagnostic recipient of reviewed metadata.user_id. Anthropic defines an opaque
external user identifier, potentially used for abuse detection. Local receipt changes that
recipient responsibility; it is neither upstream equivalence nor ADR-0116's Codex decision.
The observed Claude request also asks to keep all thinking. With no native reasoning in the
complete effective history, this one policy has an exact identity implementation.

## Decision

Adopt BC-PROTOCOL-015's closed M3/M4 shapes only for authenticated standard Messages HTTP,
nonempty matching actual GrokBuildResponses candidates, Responses upstream, CanonicalBridge.
All matching candidates must qualify for M4 consumption. An empty/mixed set, request header,
User-Agent or ordinary configuration cannot grant this responsibility. M3 must pass the actual
candidate check again before lease/send. Local controls never become native Build fields.

M3 validates the unique clear_thinking_20251015/keep:all edit and the entire valid history
on every request. Reasoning/Opaque or unrepresented native reasoning, including tool-result
content blocks, fails before send. Preserve messages, roles, order and exact tool associations.
Report context_management.applied_edits=[] on JSON and final SSE message_delta only when the
local identity rule was requested and applied. Do not invent clearing counts or upstream receipts.

M4 retains the complete raw metadata JSON value and opaque user_id string without inner JSON
parsing, normalization or truncation. It supplies no upstream user association, abuse-detection
input, authentication, session recovery or route authority. Request and companion must commit
in one existing finite queue transaction before execution. Only Persisted qualifies. Other
unsupported controls can then fail with Request-only evidence; no Attempt is manufactured.

Use RequestMetadataEvidence with an explicit Claude source, payload v2 and schema38. V1 stays
Codex-only, with unchanged serialized payload and AAD. V2's separate AAD domain explicitly binds
Messages/source as well as authenticated owner, Request, version and exclusive creation/expiry.
The encrypted record repeats the source and original Request. Readers validate that source,
payload version and durable Request protocol agree, then select only real durable Attempts.
Reuse the existing AEAD/key ring, private CLI, queue/confirmation deadline, 30-day TTL and
anti-resurrection receipts. Timeout stays UNKNOWN even if its original pending transaction
later commits; that request never sends upstream or recaptures evidence.

## Consequences

M3/M4 can coexist with text/tools/known Usage and #63's encrypted summary. M1 adaptive/display,
M2 overall effort and M5 cache control remain independently rejected, including combinations.
D2 is not selected. Original Claude compatibility remains independently BLOCKED. Codex C4,
ADR-0120 and fixed Build activation do not gain new permissions. Cost remains NOT_ADMITTED.

## Alternatives considered

- Treat metadata as Codex v1 or forward it as a Build tracking header: wrong provenance/recipient.
- Remove all extension rejection or trust a first-turn history check: admits unreviewed controls.
- Add a general evidence system: unnecessary; the Request companion already owns this lifecycle.

## Validation and rollback

Retain separate public M3/M4 REDs. Verify each control and their combination over JSON and
1/7/31-byte SSE, production compiler/factory/leased adapter and independent receiver, two related
tool cycles and follow-up, exact private readback, Usage/model/summary/terminal/accounting.
Verify pre-send zero receives for invalid histories/controls and recording faults, authorization,
replay/conflict, real restart, keys, exclusive expiry/receipts and isolated backup recovery.

Schema37 -> 38 rebuilds only the metadata table's version constraint, copying every old row,
ciphertext and expired receipt byte-for-byte. No event, Usage, ledger or old ciphertext is resealed.
New readers keep v1 reads and legacy34-36 UNKNOWN; old schema37 readers reject schema38. A
down migration refuses while any v2 row or receipt exists; it is not operational recovery.
Recover with a coherent encrypted snapshot, all referenced keys and a compatible binary; test
adoption by a new real gateway process. No production restore, paid Provider, remote write,
dependency closeout, release or deployment is authorized.
