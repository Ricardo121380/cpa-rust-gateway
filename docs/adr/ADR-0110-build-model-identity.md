# ADR-0110: Attempt-bound Grok Build model identity

| Field | Value |
| --- | --- |
| Status | Accepted |
| Date | `2026-10-18` |
| Task / Matrix / Contract references | #60; parent #58 T02/T09; ADR-0016/0100/0105/0106/0109; BC-PROVIDER-003 and BC-PROVIDER-024 |

## Context

The Build decoder previously ignored native `model`. Public Responses JSON and SSE could return
success with the route's public name even when the received model contradicted the actual outbound
model. Two public, production-factory reproductions also show that an unproven public/upstream
mapping was dispatched. An ordinary route alias is a lookup instruction, not independent native
model authority. The real historical Build alias remains UNKNOWN.

## Decision

Carry the ingress-admitted public model separately from the client's original requested alias.
HTTP Request evidence retains both. The pinned production driver compares that admitted identity
with the actual candidate and actual credential lease before building or sending a Build request.
A literal relationship requires exact, case-sensitive byte equality; whitespace normalization,
model-family guesses, current inventory and downstream response names are not evidence.

A non-literal relationship requires explicit, independently approved evidence frozen before
execution. The proof contains public/upstream/native names, source authority/reference/SHA256,
validity interval, configuration version/revision, provider/upstream/endpoint/route/candidate,
credential ID/revision, logical Client Key, channel, downstream protocol, JSON/SSE mode and Build
profile version. Source bytes must match the frozen digest. The digest proves byte identity,
not source trust: the composition owner must independently approve the source and assertions.
Proof construction performs no I/O and has no deserializer, inventory reader or implicit approval.

The production factory accepts an immutable proof set. The stock deployment composition supplies
an empty set: this change installs no real alias authority. Synthetic tests explicitly supply
synthetic approved material for their temporary accounts/configuration. Adding an operational
source of trusted real evidence requires its own approval; a route edit or catalog read cannot
create one. Binding resolves proof against the actual lease/configuration and current time,
then binds it to this Request ID. A foreign/expired/not-yet-valid/ambiguous claiming proof fails;
it cannot silently fall back to literal acceptance or another model.

The same selected model feeds the Build body and model-override header. The adapter confirms its
binding before transport and supplies one expected native name to the common JSON/SSE decode
state. Each applicable native response snapshot must provide a bounded, nonempty string model:
JSON terminal, SSE `response.created`, `response.in_progress`, `response.completed`,
`response.incomplete`, and `response.failed`. All observed phases must report the identical native
name, including when alias evidence applies. A proof for one native name cannot authorize a
mid-response rename. Validation precedes canonical/public success.

Closed, value-free `ModelIdentityRejection` reasons distinguish request incompatibility, missing
native model, invalid shape, unproven native model, phase conflict, and inapplicable alias proof.
They retain the existing 18 error categories: known pre-send incompatibility is ClientRequestError;
received native contradictions are UpstreamProtocolError. Both are non-retryable. Early failure
uses the existing HTTP error; after SSE headers commit, the existing legal failure terminal is
used exactly once, with no later success. No account/model switch, refresh, repair-and-resend or
transparent fallback is introduced.

## Consequences

This qualifies a model check, not standard metadata separation, summary suppression, native-v1,
three-client compatibility or a real Provider model relationship. Unbound standalone structural
JSON/SSE decoder APIs remain useful for historical offline projection; they cannot certify model
identity. The actual Build inference adapter always supplies a model binding.

#59's exact Usage slicing, encrypted commit confirmation, exact-owner readback, fixed TTL, keys,
idempotence and unpriced ledger semantics remain. Its fixtures now provide explicit valid models
and resolve their original client alias to a literal public/upstream name, so the original Usage
rejection is still reached. A separate negative retains the old synthetic model omission: it fails
identity before Usage validation; unread or unvalidated Usage remains unavailable, never a zero bill.
There is no read-ahead, new request, native-root persistence, inventory backfill or change to existing
reasoning ownership/continuation. Historical raw inputs, old oracle and stopped epochs are retained.

## Alternatives considered

- Treat the public route name or catalog as native authority: hides the reproduced defect.
- Accept arbitrary aliases or names resembling the selected family: lacks scoped authority.
- Re-query inventory or resend after response conflict: invents historical evidence and extra inference.
- Enforce identity only in an acceptance helper: leaves production JSON/SSE vulnerable.

## Validation and rollback

Use public Responses through production compiler/factory/auth/leased Build assembly with synthetic
credentials, an independent loopback HTTP receiver, and actual system non-loopback denial. Cover
literal and frozen alias success, independent received body/header model checks, all scope negatives,
phase conflicts, unique legal failure and zero/one receive counts with spare route attempts/candidate.
Regress #59 public durable failures and its storage/reader/CLI lifecycle separately, then run Full
and fixed-candidate Standards/Spec review. Evidence is recorded in the #60 report.

No schema migration is added. New errors may serialize the optional closed
`model_identity_rejection` field in existing event payloads; older strict readers may reject it.
Do not claim binary-only rollback compatibility. Retain coherent events/ledger/evidence and keys,
and qualify a compatible reader on an isolated snapshot before any separately authorized recovery.
Reverting code also restores the demonstrated identity gap. No deployment, push or real call is
part of this decision or its local validation.
