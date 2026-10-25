# ADR-0126: CPA v8 Messages effort extraction

> Limited successor: [ADR-0130](ADR-0130-cpa-request-controls-model-names.md) admits bridge
> budgets 0/-1, freezes candidate model normalization and adds explicit name controls. The
> original effort extraction/precedence remains; historical receipts below keep their identity.

| Field | Value |
|---|---|
| Status | Accepted |
| Date | 2026-10-24 |
| Task / Matrix / Contract references | User-approved next item in the current roadmap; ADR-0125; BC-PROTOCOL-015/016 |

## Context

The existing Messages bridge drops `output_config.effort` when `thinking` is absent and
ignores it for enabled thinking without a budget. This loses an explicit client control.
The user requested implementing the roadmap's CPA v8 effort alignment.

Reference: CLIProxyAPI **v8.0.40**, commit **d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683**,
[`extractClaudeConfig`](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/thinking/apply.go#L740).
This is a limited successor to [ADR-0125](ADR-0125-cpa-request-compatibility.md), changing
its effort-only omission and enabled-without-budget fallback. Its other decisions remain.

## Decision

Within the existing Messages → Chat/Responses `CanonicalBridge`, normalize an explicit
string effort by trimming surrounding whitespace and lowercasing ASCII. Use this precedence:

| Messages input | Projected effort |
|---|---|
| `thinking.type=disabled` | `none`, regardless of a valid explicit effort |
| `thinking.type=enabled` with a positive budget | Existing CPA budget bucket; effort does not override it |
| `thinking.type=enabled` without a budget | Explicit nonempty effort; otherwise `auto` for both targets |
| `thinking.type=adaptive/auto` | Explicit nonempty effort; existing translator fallback `xhigh` |
| No thinking | Explicit nonempty effort; otherwise no thinking field |

Explicit `none` and `auto` retain their meanings in both OpenAI targets. Existing target
effort and Reasoning capability validation applies after projection. Do not manufacture
summary intent for an effort-only request. Summary display policy remains ADR-0125's.

The scope is source effort extraction and precedence, not full CPA executor equivalence.
CPA's Chat and Codex translators have different defaults; source extraction can override
them only on applicable resolved-model paths. Unknown/user-defined paths and per-model
clamping differ. CPAR has no equivalent model-specific thinking registry in this bridge,
so it uses the explicit source control and does not reproduce that incidental path dependence.

Keep the existing closed `output_config` shape checks, known effort allowlist, positive
budget input rule (zero and negative budgets still reject), and known history/control guards.
Malformed effort shapes are not ignored even when disabled or a budget would win. This is
an explicit remaining difference from CPA's permissive extraction, not complete parity.
Native passthrough retains original JSON values, including effort whitespace and the absence
of thinking; native thinking admission and explicit `LosslessBridge` remain unchanged.

## Consequences

Effort-only and enabled-without-budget clients now communicate their intended strength to
the existing OpenAI builders. Enabled without effort changes Responses from fixed medium
to auto, matching CPA's source extraction; targets still need Reasoning capability.
No new dependency, configuration switch, schema, provider activation or routing rule is added.
Actual client, fixed-service and release qualifications remain separate.

## Alternatives considered

Keeping the old omission loses an explicit control. Copying only a target translator's
default misses the source extraction rule. Adding a general thinking registry or broadening
budget parsing is independent work and unnecessary for this bounded alignment.

## Validation and rollback

Router regression cases cover absent/adaptive/auto/enabled/disabled thinking, normalization,
empty/none/auto efforts, positive budget precedence and retained rejections. Public JSON/SSE
tests inspect actual loopback HTTP receiver bodies for Chat, Responses and native Messages;
malformed controls and orphan tool history cause no additional handoff, receive or Attempt.
Results and exact source binding belong to the
[implementation report](../reports/cpa-v8-messages-effort-20261024.md).
Rollback is the isolated increment to `compatibility::thinking` and its associated tests/docs;
preserve the inherited dirty worktree, historical receipts and unrelated frontend retirement.
