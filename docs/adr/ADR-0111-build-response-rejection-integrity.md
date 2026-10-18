# ADR-0111: Build response rejection and received failure Usage integrity

| Field | Value |
| --- | --- |
| Status | Accepted |
| Date | 2026-10-18 |
| Task / Matrix / Contract references | #61 preparation; parent #58 T03/T07/T08/T09; BC-PROVIDER-003, BC-OBS-006, BC-PROVIDER-024; ADR-0044/0100/0107/0109/0110 |

## Context

Public production-factory tests reproduce unknown response root fields returning JSON success,
an added item's numeric status being overwritten before SSE success, and already received valid
Usage disappearing from a native failed SSE terminal. These defects can be corrected without
granting any extra field separation. The [field qualification](../reports/cpar-61-field-qualification-20261018.md)
remains incomplete; #61 cannot close on synthetic standard successes.

## Decision

Use one Build root validator in the common JSON/SSE decode state. Accept only fields consumed by
the existing decoder: id, model, object, status, output, usage, incomplete_details and error.
Validate the fixed object discriminator, output/status shapes, and applicable error/incomplete
details. Other root fields, including timestamps, metadata and echoed controls without an
approved representation, fail closed. These are not newly separable metadata. This intentionally
rejects responses previously accepted by silently discarding such fields.

SSE created/in-progress snapshots must match that phase when status is supplied and may not
silently carry nonempty output or Usage that this phase cannot consume. Terminal snapshots keep
their existing identity and completed/incomplete checks; a failed snapshot cannot claim another
status. The JSON terminal reuses the common root schema without pretending to be an SSE start.

Validate supplied Build item status before native item metadata can normalize an added item.
The existing completed-item, tool-argument, part, identity, index/order and terminal checks stay
in force. This change is Build-local and does not grant behavior to Official/Console or Messages.

Parse already received Usage on a native SSE failed terminal before emitting its safe failure.
Known counters follow the existing Attempt-bound observation path; extra Usage remains rejected
and follows #59's exact raw-slice AEAD confirmation and authorized reader. A failure cannot become
success, read another frame, retry, change candidate/account/model, or invent pricing. A later
success tail is deliberately left unread. If that tail already arrived in the same transport
chunk, decoding stops at the validated failure and commits its Usage instead of rolling it back.
The source stays finished after emitting that failure. Success decoding retains its complete-chunk
validation, including conflicting terminal checks. Completeness describes the received Usage
object only.

## Consequences

The fixes improve strict failure correctness while additional metadata qualification and successful
metadata retention remain BLOCKED. Standard JSON/SSE tool/summary regression evidence is separate
from that missing capability, actual clients and real Provider acceptance. Native failed JSON
evidence and full Build cancellation/timeout qualification are not added by this decision.

## Alternatives considered

- Ignore unknown root fields or normalize arbitrary status values: hides the reproduced failures.
- Treat saved JSON, a synthetic allowlist or generic public xAI docs as semantic authority: violates
  ADR-0107's actual-field qualification.
- Read more frames or retry to obtain Usage: changes execution and cancellation semantics.

## Validation and rollback

Use the public Responses handler, production compiler/factory and actual Build adapter with
synthetic identities, an independent loopback TCP receiver and loopback-only network denial.
Retain RED/GREEN for root/status/failure Usage, unique legal SSE failures, no tail read and exact
encrypted readback/ledger. Regress #59/#60 and existing standard tools/summary; run Full and fixed
baseline/candidate Standards and Spec reviews. Final results belong in the
[#61 report](../reports/cpar-61-responses-20261018.md), not in a completion claim here.

No schema, evidence format, reader, key, TTL, dependency or release change is introduced. Existing
schema-35 recovery constraints remain. Reverting restores the demonstrated strictness/Usage bugs;
it does not qualify the blocked metadata rule. Production rollback/deployment is not authorized.
