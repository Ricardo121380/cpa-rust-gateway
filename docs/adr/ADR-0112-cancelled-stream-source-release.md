# ADR-0112: Release cancelled stream sources before local recording finishes

| Field | Value |
| --- | --- |
| Status | Accepted |
| Date | 2026-10-18 |
| Task / Matrix / Contract references | #61 continuation; #58 T03/T06/T08/T10; ADR-0016/0109/0111; BC-PROVIDER-003 and BC-OBS-006 |

## Context

A public HTTP reproduction uses the production compiler/factory, leased Build adapter and an
independent TCP upstream. It holds a real encrypted Usage transaction's acknowledgement at the
event port. A disconnected caller cannot release the upstream response and single-capacity
credential while the stream pump waits for that acknowledgement. Source lifetime currently
extends through local recording even when the response has already failed.

The field decision is independent: the concrete cost candidate remains NOT_ADMITTED after the
user reviewed the [channel draft](../reports/cpar-61-cost-contract-draft-20261018.md). Successful
metadata separation and its fault matrix remain blocked. Rejected Usage can still be retained
under #59 without granting success.

## Decision

The existing HTTP SSE pump explicitly owns an optional source. When a terminal event or source
error has been obtained, remember its already validated buffered Usage and drop the source before
awaiting local confirmation or delivering the terminal. This releases the owned socket and actual
credential lease. Normal cancellation likewise drops the source before its final Usage flush.

If cancellation arrives while an event's existing Usage confirmation is pending, remember the
source's already decoded Usage, drop the source immediately, then finish that same confirmation.
Do not cancel and recreate its future or enqueue a second companion. The existing queue/writer
remains bounded and owns the accepted transaction; its existing timeout still bounds production
confirmation. No additional task, queue, source read or Provider operation is introduced.

The delivery owner still determines Request success, failure or cancellation. A source Attempt
that started successfully is not proof of completed Request delivery. A cancelled Request cannot
be made successful by a later Usage commit. Received validated counts retain their exact Attempt
and one source/ledger identity; unread/unvalidated Usage remains unavailable, not zero. A retained
`complete_usage_object` describes the received object, not the cancelled response or Provider bill.

The correction is in the shared SSE pump, so the existing Responses, Chat and Messages pump
regressions apply. It changes resource lifetime, not a channel's metadata allowance, protocol
representation, summary policy or retry rules. JSON transport and fixed Build profile stay intact.

## Consequences

Network and credential ownership can end while bounded local recording is still resolving. This
does not allow downstream success before persistence. Late failures still use one legal terminal
after committed headers; cancellation sends no invented success and performs no evidence read-ahead.
The writer, schema 35, sensitive payload, reader, key ring, TTL and billing rules are unchanged.

## Alternatives considered

- Keep the source until local recording finishes: reproduces avoidable occupied capacity after
  cancellation or known failure.
- Drop the confirmation future and flush again: can lose an unacknowledged result or replay a
  submission; the original transaction must keep its own identity and bounded outcome.
- Read more upstream or issue another request for Usage: changes cancellation and retry semantics.
- Admit cost metadata to obtain a success sample: the user explicitly retained NOT_ADMITTED.

## Validation and rollback

Public production-Build tests cover body cancellation and an abortive real client socket reset,
pending standard and encrypted-rejection Usage receipts, actual lease 1 -> 0, independent upstream
EOF/reset, explicit later capacity reuse, HTTP truncation, well-framed premature SSE EOF, and real
UpstreamClientPool idle/total deadlines. First-party field research, controlled protocol tests,
actual clients and real Provider evidence remain separate. The exact same-Attempt encrypted
conflict uses the actual writer and verifies original authorized readback and one ledger row.

Regress #59/#60, the previous failed+tail same-chunk Usage case, shared HTTP streaming and native
Build decoders, then Full and fixed-baseline Standards/Spec review. Results and historical RED/
harness failures are recorded in [the continuation report](../reports/cpar-61-continuation-20261018.md).
No migration or rollback operation is needed for this code correction; reverting it restores the
resource-lifetime defect. Existing schema-35 evidence recovery restrictions remain in force.
This decision authorizes no deployment, push, real inference, #62/#63 or other-ticket change.
