# ADR-0123: First-release Claude D2 restricted support

> **2026-10-23 request-policy successor:** [ADR-0125](ADR-0125-cpa-request-compatibility.md)
> adopts CPA-style request compatibility. Conflicting universal rejection / M1-M2-M5 blockers
> and effort-negotiation requirements below are historical; other invariants and receipts remain.

| Field | Value |
| --- | --- |
| Status | Accepted |
| Date | 2026-10-23 |
| Task / Matrix / Contract references | #58/#64/#66; #58 T01–T03/T05/T08/T10–T12; [BC-PROTOCOL-016](../contracts/BC-PROTOCOL-016-claude-restricted-candidate.md), ADR-0108/0114/0121/0122; #67 remains a separate gate |

## Context

The user has explicitly selected the verified Claude D2 configuration for the first release.
The [D2 evaluation](../reports/cpar-64-d2-candidate-20261022.md) established actual-client
PASS_LOCAL_CONTROLLED on code `4015bd4542dbc52fde8984c14c01b34843ef3c59`.
ADR-0122 deliberately authorized only that independent experiment. Its original decision and
the reports' NOT_DECIDED statements remain accurate historical records. This later decision
records product adoption; it supplies no new runtime or fixed-service evidence.

## Decision

Select unmodified **Claude Code 2.1.302**, entry `/Users/huangrui/.local/bin/claude`, resolved
binary `/Users/huangrui/.local/share/claude/versions/2.1.302`, SHA-256
`5262ce9de86243fed0d21edc39b271555d56780d0112e2b194439fe1c6662d37`, in the evaluated
isolated `--bare --print` Messages configuration with literal `grok-4.5`. All four controls
are mandatory together:

```sh
DISABLE_PROMPT_CACHING=1
CLAUDE_CODE_DISABLE_THINKING=1
CLAUDE_CODE_MAX_RETRIES=0
CLAUDE_CODE_DISABLE_NONSTREAMING_FALLBACK=1
```

Reuse the [existing Messages usage guide](../support/grok-build-messages.md) and acceptance
driver. Examples use placeholders, local process configuration and a disposable work directory;
they do not change global settings or initiate requests. Other versions, default retries,
non-streaming fallback and other client shapes do not inherit the evaluated result.

Thinking omission describes the request, not whether the model reasons. The initial effort
request still receives BC-PROTOCOL-016's accurate local rejection before send. Only the client
can negotiate by issuing a new authenticated Request without effort. CPAR never deletes the
field, fabricates an upstream error or transparently resends a modified request. The rejected
Request has zero receives/leases/Attempts/Usage/ledger; legitimate M4 evidence is Request-only.

Standard Messages does not display Grok summary. Complete encrypted summary, item/part
identity/order, exact owner/Request/Attempt readback, tool results and continued visible history
remain required under ADR-0114. Missing body remains missing; no thinking or signature is invented.
The M3/M4 contracts are unchanged. D2's captured omission of context_management does not prove
that the actual client exercised M3.

The first release still requires **Codex, Claude Code D2 and Pi, all through Grok Build**, to
complete the agreed tool loops, follow-up, failure and cancellation behavior at their applicable
gates. Product adoption does not mean first-release READY, #64/#66 completion, fixed Build
activation, real Provider acceptance or release/deployment.

Original Claude adaptive/display (M1), overall effort (M2) and block prompt-cache (M5) requirements
remain open follow-up work under #58/#64. They are outside the selected first-release D2
configuration; they are neither removed nor completed. Its original BLOCKED_BEFORE_FIRST_RESPONSE
branch and every retry/fallback RED remain evidence. No promise of original-configuration
compatibility follows from selecting D2.

## Consequences

- [The candidate support inventory](../support/first-release-candidates.md) separates product
  selection, actual-client controlled results, binary/OCI lifecycle, fixed Build qualification
  and real Provider acceptance. Different tested sources retain their identities.
- Formal native-field qualification and stock capabilities are unchanged. No request, header,
  User-Agent or ordinary configuration grants missing qualification. controlled-test-support
  stays test-only and must not enter delivered artifacts.
- #64 → #63 → #61, #66 → #62/#64/#65 and #67 → #66 remain native dependencies. This local
  documentation change neither closes tickets nor changes GitHub state.
- #61 cost_in_usd_ticks remains NOT_ADMITTED; costs remain UNKNOWN without independent evidence.
  Wait for new primary evidence rather than repeating the existing research.

## Alternatives considered

- Keep D2 as an experiment only: superseded for product selection by the user's explicit choice.
- Require original adaptive/display, effort and block cache support before this first release:
  retained as later obligations instead of the selected D2 configuration's release conditions.
- Remove fields at the proxy, fabricate an upstream rejection or enable test qualification in a
  delivery build: violates the unchanged protocol and production qualification boundaries.

## Validation and rollback

This is a specification/configuration-expression change with no product implementation, schema,
lock, capability or artifact change. Check document links, contract references, all four settings,
client/source/status consistency, issue drafts and the owned diff. Historical Full, actual-client,
reader and OCI results are references, not reruns. Future fixed-service activation needs its own
qualified contract/evidence and immutable candidate; #67 additionally needs explicit account,
environment, fresh Pin and bounded new-epoch authorization. Reversing this product choice would
require a later decision; it must not erase this record or rewrite the experimental history.
