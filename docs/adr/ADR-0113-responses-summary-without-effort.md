# ADR-0113: Responses summary requests without an explicit effort

| Field | Value |
| --- | --- |
| Status | Accepted |
| Date | 2026-10-19 |
| Task / Matrix / Contract references | #62; #58 T02/T03/T08; BC-CORE-002, BC-ROUTER-004, BC-PROVIDER-003; [BC-PROTOCOL-009](../contracts/BC-PROTOCOL-009-codex-responses-controls.md) |

## Context

The actual isolated Codex 0.161.0 request contains `reasoning:{"summary":"auto"}` with no effort.
The old Responses decoder requires effort, independently of its earlier namespace rejection.
OpenAI's summary and effort controls are independent. The fixed Codex source represents both as
optional fields, and the frozen public Grok client also constructs a summary with absent effort.
The field contract records exact sources, their scope, and the missing production release mapping.
No source establishes a particular effective Grok effort when the client omits it.

Core `Thinking` deliberately means an explicit effort and is embedded in encrypted stored-response
payload version 1. Making effort nullable would change that established invariant and reader shape
for one protocol control. Existing raw extensions already preserve reviewed protocol controls.

## Decision

Retain a summary-only request as `CanonicalRequest.extensions["openai.responses.reasoning"]`,
containing the object with its sole `summary` member, while `thinking` remains absent. Accept only
`auto`, `concise`, or `detailed`. Empty objects, explicit null effort, unknown members, duplicate
names and unsupported summary values are rejected. Effort-bearing requests retain their existing
typed Thinking representation and semantics.

Responses-to-Responses routing validates this exact shape and requires the existing Reasoning
capability, including exact passthrough. Typed Thinking plus this root control is a collision.
Chat and Messages transformations reject it; no effort, budget, disabled mode or summary deletion
is an equivalent. The generic Responses and fixed Build encoders revalidate the retained control
and emit the same summary-only object. Reserved-field collision protection remains.

This proves faithful request construction and controlled standard JSON/SSE delivery. It does not
establish acceptance by the production Build 1.0.50 service, its effective default effort, a new
capability, or real Grok usability. The public source-to-release mapping remains unproven.
Namespace, hosted web search and Codex client_metadata do not gain admission from this decision;
their concrete missing qualifications and current rejection behavior remain in BC-PROTOCOL-009.

## Consequences

No Canonical field, Thinking field, SQL schema, payload version, AAD, key or TTL changes. Old
effort-bearing Canonical bytes retain their serialization. New records use the existing raw
extension envelope; reopen, exact-owner lookup, key rotation and identical replay are tested.
Readers understanding version 1 can retain that envelope, but older execution code rejects the
new control. Readability therefore does not imply old-binary execution compatibility. Preserve
coherent state and all referenced external keys for any separately authorized recovery.

The unmodified Codex request remains blocked by its other required fields. A new explicit CLI
success gate cannot treat expected HTTP400 preparation as compatibility. #61, its NOT_ADMITTED
decision, and #62's formal dependency and complete acceptance criteria remain unchanged.

## Alternatives considered

- Add an effort default: changes the request and lacks an applicable source.
- Remove summary or disable thinking: loses an explicit client control.
- Make every Thinking effort optional: expands core/persistence shape unnecessarily.
- Admit arbitrary reasoning root JSON: bypasses reviewed controls and collision checks.
- Flatten namespace or convert hosted search to a function: loses distinct tool semantics.

## Validation and rollback

Retain the public Build summary-only HTTP400 RED. Verify JSON and fragmented SSE GREEN with
independent TCP receives, original absent effort, summary identity, known Usage, two ordered tool
cycles and follow-up. Exercise capability, bridge, collision, malformed-control and other-field
pre-send refusals, plus unqualified namespaced output refusal without retry. Run the unchanged
fixed actual CLI path separately; its success gate remains FAIL while Codex is blocked. Regress
Pi and #59/#60/#61, then bind Full and Standards/Spec reviews to the candidate source/manifest/lock.

Rollback restores the old summary-only rejection. No migration or state deletion is needed;
retained records and historical failures must remain. No deployment, real account, Provider call,
new request budget, client installation change, or other issue advancement is authorized here.
