# ADR-0105: Source-bound observation of response projection failures

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-10-11` |
| Task | CPAR-38 / #47; delivery #57; parent #9 |
| Contracts | [BC-HTTP-001](../contracts/BC-HTTP-001-actix-responses-handler.md), [BC-HTTP-002](../contracts/BC-HTTP-002-actix-chat-completions-boundary.md), [BC-PROVIDER-003](../contracts/BC-PROVIDER-003-grok-build-responses-boundary.md), [BC-ROUTER-004](../contracts/BC-ROUTER-004-protocol-transform-admission.md), [BC-OBS-004](../contracts/BC-OBS-004-default-deny-log-redaction-and-body-sampling.md) |

## Context

The preceding isolated Chat JSON request returned 502 after a successful Attempt and
a failed Request. It lacked Request-ID; its raw upstream response was not captured.
Synthetic public-entry regressions independently prove missing admitted Request-ID
and loss of already decoded Build Usage behind a rejecting client projection. They
do not identify the uncaptured real response's fields or establish zero cost.

## Decision

Keep the 18 existing error categories and HTTP statuses. A response projection failure
retains one of the existing six closed rejection reasons in `GatewayError`; fixed
public text names the projection stage and the restriction. The router re-exports
that core reason type. Safe structured logging contains only stage, reason and the
established gateway Request ID. No native values, arguments or content enter errors.

For Chat, Messages and Responses JSON/SSE, attach the existing gateway Request-ID to
responses only after confirmed Request admission. Do not echo a client ID, create an
identity for authentication/model resolution/admission failures, or turn an SSE error
after committed headers into a fabricated HTTP502 or successful terminal frame.

Provider event sources may expose a validated Usage snapshot still buffered in the
current attempt without reading upstream. Build buffered JSON and pending SSE events
and both native/runtime wrappers forward it. The existing HTTP failure/cancellation
flush merges snapshots non-additively, preserves unknown fields and binds the confirmed
Attempt. Existing store idempotence and ledger source-event ownership apply. Usage
that has not arrived or failed strict decode is unavailable; no estimate is introduced.
An upstream Attempt success does not make final Request delivery successful.

Private capture is off unless all three process settings are explicit:
`CPAR_BUILD_DIAGNOSTIC_DIR`, `CPAR_BUILD_DIAGNOSTIC_REQUEST_SHA256` and
`CPAR_BUILD_DIAGNOSTIC_SOURCE_REVISION`. The revision must equal the embedded candidate;
the public payload hash must exactly match a Chat non-streaming request. Other requests,
including Pin, are not captured. An exclusive file reserves one capture even if transport
fails. Capture uses a canonical existing 0700 directory descriptor and exclusive,
single-link, same-owner 0600 files. It wraps the pull-only Build body, forwards the same
egress Attempt once and retains at most the existing 1 MiB bound. It captures classified
response headers, never outbound headers, credentials or the public request body.
Partial bytes remain private on failures or interruption. Capture errors stop that
execution; they cannot reopen the epoch or initiate retry. No raw content is logged,
committed, posted, or sent to another model. Only the authorized synthetic request may
be selected, in a new owned protected epoch.

Strict equivalence in ADR-0037 and retained native semantics are unchanged. Existing
plain reasoning and Tool conversions remain eligible; rich summaries, citations,
signatures or native structure without a proven equivalent continue to reject. Any
change to that rule requires a separate focused decision.

## Consequences

New error JSON can contain optional `projection_rejection`; old category-only JSON is
unchanged and readable. This adds no SQLite schema or writes to historical events.
Older strict readers may reject new StreamError payloads containing this field. A
future deployment or rollback must therefore follow ADR-0100/0103's coherent state
and reader qualification; this local batch grants no binary-only rollback or release
permission. New private diagnostic files are outside durable public data and remain
excluded from Git and external review inputs.

The real cause still needs source-bound evidence. A new captured sample can qualify
only its own epoch. Neither a local strict fixture nor successor Pin can replay or
qualify the previous uncaptured request or the entire six-combination matrix.

## Alternatives considered

- Free-form error details or native body logs admit arbitrary sensitive values.
- Early or client-supplied Request-ID loses confirmed ownership.
- Invented Usage or rewritten Attempt outcomes corrupt lineage and cost semantics.
- Reading ahead after rejection to discover Usage adds unrequested upstream work.
- Dropping native items or reasoning to obtain a success loses retained semantics.

## Validation and rollback

Public runtime-factory regressions exercise all three protocols in both modes with
complete, partial and absent Usage; duplicate confirmed Usage must yield one durable
source event and one unpriced ledger row. Error frames are value-free, including when
representable stream data was delivered before a later rejection. Unauthenticated
requests have no fabricated ID or Attempt. Capture tests cover exact selection,
source/mode/protocol mismatch, private bytes/modes, unsafe directory rejection and
no second Provider send. Existing protocol/router regressions retain strict and
equivalent conversions, call_id, ordering and history. Task/Phase and independent
Standards/Spec reviews bind an immutable commit before any real diagnostic eligibility.

Rollback of code alone restores generic errors/missing headers/buffered Usage loss;
compatibility of already written new payloads must be separately established before
any deployment. No production configuration or frontend change belongs to this decision.
