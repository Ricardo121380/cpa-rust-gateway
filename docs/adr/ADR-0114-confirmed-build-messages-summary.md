# ADR-0114: Confirmed Build summary retention for standard Messages

| Field | Value |
| --- | --- |
| Status | Accepted |
| Date | 2026-10-19 |
| Task / Matrix / Contract references | #63; #58 T02/T03/T05–T11; ADR-0100/0107/0108/0112/0113; BC-OBS-006; [BC-OBS-007](../contracts/BC-OBS-007-messages-summary-evidence.md) |

## Context

The accepted #58/#63 delivery boundary permits standard Messages to omit Grok Build reasoning
summary from client display after complete encrypted retention and a demonstrated tool loop.
This is a reasoning-specific exception, separate from metadata admission and NativeUsageEvidence.
The public JSON/SSE RED rejects summary through the production compiler/factory, leased adapter
and real bounded upstream HTTP pool. Cost remains NOT_ADMITTED and real Claude Code belongs to #64.

## Decision

Enable the exception only after actual selection of `GrokBuildResponses`, a Messages consumer
and `CanonicalBridge`. `CanonicalBridge` on this precise path means the explicitly limited
standard delivery contract below; it does not claim complete reasoning equivalence. The generic
projector, LosslessBridge and exact Passthrough keep their existing meaning and admission rules.
There is no User-Agent dispatch, generic reasoning suppression, native-v1 envelope or new mode.

The native decoder continues to validate the original item/part stream, done snapshots, order,
model and terminal. A composition-owned collector validates the complete original Canonical
lifecycle and consumes only reviewed summary item/delta events. Other events pass through the
unchanged target projector and tool constraints. Reasoning bodies, ciphertext, signatures,
citations, unknown parts and unqualified Usage do not gain omission rights. Responses continues
to deliver summary; Chat and other adapters do not inherit this exception.

Retain ordered completed reasoning item snapshots, their positions among all output items,
native response identity, JSON/SSE source, local confirmation time and explicit completeness.
Part order is its array index; item ID, type and status remain in each complete item. Missing body
remains missing, explicit empty body remains empty. This is the confirmed decoder representation,
not an assertion of original whole-wire bytes. Strings and their hierarchy are complete; there
is no numeric reasoning metadata to round. A 64 KiB aggregate bound rejects instead of truncating.

Use a distinct core MessagesSummaryEvidence and schema-36 table. The request-local sink retains
the actual confirmed Attempt and binds the companion to it. Its idempotent Attempt replay anchors
one existing bounded queue slot/transaction, creates no new Attempt row, and is not exported again
to telemetry or the stage ledger. The worker verifies durable Request/Attempt/indexes, original
execution checksum, actual `grok.build` channel, Messages source transport and owner. No summary
enters GatewayEvent, NativeUsageEvidence, UsageSummary, quarantine, audit or billing.

The writer seals v1 with the existing SecretStore/XChaCha20-Poly1305 and configured external key
ring. Domain-separated AAD authenticates exact owner/Request/Attempt, payload version and exclusive
creation/expiry. The encrypted payload repeats the original Attempt/execution and evidence. Commit
of the complete companion precedes Persisted. Queue-only, disabled, missing writer/key, storage,
encryption, identity conflict and unconfirmed receipts all fail client success. Same evidence
replay authenticates and compares; changed evidence conflicts and cannot renew the 30-day TTL.

Evidence confirmation time belongs to the original Request lifetime. A pending local transaction
may finish after cancellation or a failed delivery, without recapturing historical content. New
captures after a finished Request or after interrupted recovery fail. CompletedResponse labels
native confirmation only. ConfirmedItems labels just the listed done items on an incomplete
response; unfinished parts remain unknown. No reader or cleanup reads ahead or reissues inference.

The shared HTTP pump snapshots already-decoded evidence and drops the upstream source/lease
before terminal confirmation. Cancellation while recording awaits the same in-flight future,
never resubmits it and never converts cancelled delivery to success. Known Usage retains its
existing independent, non-additive source/ledger identity. Missing cost and Usage stay UNKNOWN.

Provide the separate read-only `SqliteMessagesSummaryReader` and local
`gateway messages-summary-evidence` command. Reuse the existing explicit owner/Request/Attempt,
external key directory and exclusive descriptor-relative 0600 output in a canonical 0700
directory. Missing/foreign/expired/historical data is UNKNOWN; invalid authentication or lineage
fails. There is no new management endpoint, stdout plaintext or frontend change.

## Consequences

The supported controlled continuation is explicit standard Messages history: clients replay only
the visible text and tool calls/results. No helper restores hidden summary, and retained diagnostics
are not a continuation service. Modes requiring unrepresented reasoning, native identity replay,
server continuation or incompatible thinking/budget remain rejected. Controlled loopback success
does not establish the fixed production Build service's continuation requirements or Claude Code
compatibility. Their qualifications remain separate, including #61's actual field/profile blockers.

## Alternatives considered

- Remove summary in the generic projector: loses Provider/mode qualification and durable confirmation.
- Reuse NativeUsageEvidence or place summary in Usage: conflates reasoning with token/billing evidence.
- Add an unbounded writer, synchronous SQL in HTTP or a second confirmation task: changes established
  bounds and cancellation ownership. The existing finite queue and blocking worker suffice.
- Claim full LosslessBridge or inject summary into client history: violates the accepted display and
  evidence boundary and cannot prove a standard-client tool loop.

## Validation and rollback

Public Messages JSON and 1/7/31-byte incremental SSE use the actual compiler/factory/leased Build
adapter and bounded HTTP pool with an independent loopback receiver. Verify two related tool
cycles, replies/follow-up, exact outgoing visible history, model, known Usage, complete authorized
summary readback, legal unique terminal and ledger identities. Retain pre-send zero-receive and
post-send one-receive failures, unknown/empty/conflicting summary, write/key/confirmation faults,
pending receipts, actual socket FIN/RST, EOF/deadlines and Responses/Chat/other-provider regressions.

Schema 35 -> 36 creates an empty independent namespace; historical events and Usage are unchanged.
New readers accept schema 34/35 as UNKNOWN for summary. Old binaries/readers cannot be assumed to
accept schema 36. Recovery requires a coherent SQLite snapshot with events/execution/summary/ledger,
all referenced external key versions, and a compatible reader; verify reopen, separate reader
processes, key rotation, exclusive expiry and an isolated encrypted restore. Never operationally
drop the table or restore only an old binary. This ADR authorizes no production restore, real
Provider, new paid budget, deployment, release or change to GitHub dependencies.
