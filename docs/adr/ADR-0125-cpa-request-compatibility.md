# ADR-0125: CPA-style request compatibility

> Limited successor: [ADR-0126](ADR-0126-cpa-v8-messages-effort.md) changes only Messages
> effort-only extraction and enabled-without-budget precedence/fallback on 2026-10-24.
> The original decision and source identity below remain as historical context.

> Operator-configured bounded target options and deliberate filters are defined separately by
> [ADR-0134](ADR-0134-versioned-request-rules-upstream-headers.md); empty policy retains this behavior.

| Field | Value |
|---|---|
| Status | Accepted |
| Date | 2026-10-23 |
| Task / Matrix / Contract references | User request to relax request admission using CPA; ADR-0037, ADR-0100, ADR-0121, ADR-0122, ADR-0123; BC-PROTOCOL-015/016 |

## Context

Normal client options were rejected even when a native upstream could receive them or CPA had
an established conversion. The user explicitly chose practical CPA compatibility over requiring
every request conversion to be mathematically lossless. This supersedes the conflicting request
admission rules in the references above; their historical receipts remain bound to their old code.

Reference: CLIProxyAPI **v7.2.158**, commit **5b2785617d1e7de84a9f4dee599d275a4ccd8999**.
The implementation follows `sdk/translator/registry.go`, `internal/thinking/convert.go`,
`internal/thinking/summary.go`, `internal/thinking/provider/claude/apply.go`, and the
`internal/translator/{openai,codex}/claude` request converters. The upstream MIT notice is retained
in [THIRD_PARTY_NOTICES](../../THIRD_PARTY_NOTICES.md).

## Decision

Use the existing modes, with no new flag or dependency:

- **Passthrough**: same protocol and a complete native body remain required. Preserve unfamiliar
  root, message and content options. Native builders replace the selected model and serialize the
  body; this promises JSON values, not identical whitespace or bytes. Validate known controls,
  tool correlation and known signed thinking history independently of unknown options.
- **Canonical / same-protocol CanonicalBridge**: allow target-native root options to reach the
  existing encoder. Existing content, collision and execution-control validation still applies.
- **Cross-protocol CanonicalBridge**: perform the explicit compatibility mappings below.
  Unmapped source-specific root options are omitted like CPA's fresh target templates. Local
  private metadata, structured-output controls, native tools, content identities and unknown
  content do not become disposable root hints.
- **LosslessBridge**: keep its explicit lossless contract, including rejection of approximations.

Messages → Chat/Responses maps enabled budgets to CPA effort buckets: 1–512 minimal,
513–1024 low, 1025–8192 medium, 8193–24576 high, and larger values xhigh. Disabled maps to none.
Adaptive/auto uses trimmed, lowercased `output_config.effort`, defaulting to xhigh. Enabled without
a budget uses medium for Responses and auto for Chat. An effort option without thinking does not
enable reasoning: it is accepted and omitted, leaving the upstream default unchanged. Unlike the
Codex-specific CPA template, CPAR does not inject medium into requests that omitted thinking.

Summary intent is independent: summarized display maps to Responses summary auto; omitted or
unrecognized display does not request a summary. Responses summaries map to Messages summarized
display; Chat reasoning requests summary auto on Responses. Explicit none/disabled wins. Messages
summary-only translation activates a minimum manual budget, without inventing qualitative effort.
OpenAI → Messages manual budgets follow CPA's table, bounded to at least1024 and strictly below
the caller's output limit. A limit too small for that representation rejects. CPAR lacks CPA's
per-model numeric range/adaptive registry here and does not claim model-specific clamping parity.

Messages block-cache hints on system/text, tool declarations, tool calls and results are omitted
when targeting OpenAI. This does not claim upstream caching. Native Messages retains their exact
placements, including mixed lifetimes; a mixed set has no single canonical retention summary.
OpenAI cross-protocol cache-key identity still requires an explicit mapping and remains rejected.
Common temperature/top_p and stop options are mapped where the reference has target fields;
Messages temperature takes precedence over top_p, and a Chat stop string becomes a Messages array;
Messages → Responses follows the reference's omission of those sampling/stop hints.

Native Anthropic candidates receive native metadata/context_management. This is separate from
Build's local M3/M4 consumption: mixed recipient sets still reject, Build still receives no private
Messages metadata, and its encrypted receipt must commit before execution. Native metadata cannot
grant owner, model, credential or routing authority.

## Consequences

The normal bridge accepts adaptive/display, effort and prompt-cache combinations without first
requiring Claude to negotiate away those fields. D2's historical effort400 and its old request
counts are not current acceptance expectations. Actual stock Claude and fixed-service acceptance
must be rerun before extending their support claims. This decision changes request compatibility,
not the separate encrypted summary response policy, Usage accounting, auth, tenant ownership,
SSRF, retries, provider qualification or release status.

## Alternatives considered

Keeping universal exactness would retain the incompatibility the user asked to remove. Dropping
all validation would conflate vendor options with executable controls and history identities.
Adding another strict/compatible configuration switch is unnecessary: the existing modes express
the distinction.

## Validation and rollback

Regression tests cover budget boundaries, independent summary mapping, native JSON preservation,
mixed cache lifetimes, independent HTTP receivers, JSON/SSE and five requests containing two tool
cycles. Bad auth, malformed controls, orphan tool results, signed history, metadata ownership and
zero-send boundaries remain tested. Workspace checks use the current inherited worktree overlay.
No real Provider, actual client, remote CI or deployment result is inferred from synthetic peers.
No schema migration is introduced; revert this isolated source increment to restore the prior
admission policy, preserving unrelated architecture/frontend work and all historical evidence.
