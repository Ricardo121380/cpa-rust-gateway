# ADR-0122: Isolated Claude Code restricted candidate

| Field | Value |
| --- | --- |
| Status | Accepted |
| Date | 2026-10-22 |
| Task / Matrix / Contract references | #64 D2 candidate only; #58 T01/T02/T03/T05/T06/T08/T10; ADR-0114/0121; [BC-PROTOCOL-016](../contracts/BC-PROTOCOL-016-claude-restricted-candidate.md) |

## Context

The user selected an independent, isolated D2 evaluation. This does not adopt D2 for the first
release, amend #58/#64, or qualify the fixed Build service. The installed unmodified Claude
must itself form the request; a synthetic Messages caller cannot substitute for it.

## Decision

Use the current frozen Claude executable, isolated HOME/config/workdir, synthetic credentials,
grok-4.5 identity, public Messages authentication and production compiler/factory/leased Build
adapter. Only an independent controlled loopback peer substitutes for the external service.
Evaluate DISABLE_PROMPT_CACHING=1 and CLAUDE_CODE_DISABLE_THINKING=1. Prove their wire effect;
omission of thinking says nothing about whether the model reasons by default. D2 promises no
adaptive/display, overall effort or block prompt-cache semantics. M3/M4 and #63 remain exact.
Set the documented CLAUDE_CODE_MAX_RETRIES=0 for bounded ordinary failure handling, after
preserving the actual default-policy retry failure. Verify capability negotiation still works.
Also set the documented CLAUDE_CODE_DISABLE_NONSTREAMING_FALLBACK=1 after preserving the
MAX_RETRIES-only extra-send failure. This separately disables the mid-stream fallback.

When a complete nonempty matching candidate set proves that the sole remaining conversion
rejection is Messages output_config.effort, return the fixed, secret-free Messages 400 specified
in BC-PROTOCOL-016. Identify the local gateway conversion as the rejecting party. Never pretend
the upstream returned it. Preserve other independent rejections and unknown fields. Empty,
missing, mixed or compatible candidate evidence cannot prove this specific rejection.

The error may trigger the client's documented effort recovery. CPAR never removes a field from
the original request and never resends a modified request. A subsequent client HTTP request has
its own authenticated Request and, only if actually dispatched, its own Attempt/Usage/ledger.
The rejected Request has no lease/Attempt/Usage/ledger; valid M4 retains Request-only evidence.

Retain the original client branch. D2 success requires two related real tools, continued history,
follow-up, strict native success, summary/metadata readback, real failure and client-alive cancel.
If the current client cannot produce the exact candidate, retain RED and stop dependent positives.

## Consequences

This is an independent candidate result, not original Claude compatibility or first-release
admission. Existing Accepted decisions remain historical facts. No new field qualification,
schema, metadata source, cost admission, real credential or Provider budget is granted.

## Alternatives considered

- Capability declarations in Base URL mode: not applicable per current official documentation.
- effort=auto as field omission: not its documented meaning.
- Claude model aliases, cloud-provider mode, modified clients or proxy field removal: change the
  subject of this evaluation and do not prove D2.
- Guessing different error messages until success: not evidence of an accurate rejection.

## Validation and rollback

Preserve original-client and configured-client raw REDs and the specific-diagnostic public RED.
Regress independent/combined M1/M2/M5, unknown fields, M3/M4, JSON/SSE, summary and strict Codex/Pi.
Final Full and actual-client evidence bind immutable code/lock, explicit inputs and inherited
overlay. No new persistence format is introduced. Fixed service, real Provider, CI, release and
deployment remain independent. No GitHub write, push, global client configuration or paid request.

## Later product decision (2026-10-23)

[ADR-0123](ADR-0123-first-release-claude-d2-restricted-support.md) subsequently selects the
verified four-variable D2 configuration for the first release. The experimental authorization,
results and non-adoption statements above retain their original historical scope. The later
selection does not qualify the fixed Build service or original Claude configuration.
