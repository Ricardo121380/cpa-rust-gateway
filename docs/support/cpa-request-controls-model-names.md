# CPA request controls and model names (#69)

Reference: CLIProxyAPI **v8.0.40**, commit
`d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683`. Model control metadata is frozen in
`crates/gateway-router/src/protocol_transform/cpa_model_thinking.json`, a projection of that
commit's `internal/registry/models/models.json` (Claude and Codex entries only). It does not
refresh from the floating models repository and does not establish credential entitlement.
The existing compiler capability and access-group route grants still apply.

## Request controls P0

The three stages are independent: retain and validate source controls, normalize for the
selected candidate's actual upstream model, then write target protocol fields. A public alias
never supplies a model capability. ADR-0126 extraction is reused unchanged except for admitting
signed budgets 0/-1 in Messages compatible conversion. Malformed types still fail, including
malformed output_config when disabled or a budget would win.

| Messages source | Extraction before model normalization |
|---|---|
| disabled | none; wins over valid budget and effort |
| enabled + budget 0 / -1 | none / auto; wins over effort |
| enabled + positive budget | 1–512 minimal; 513–1024 low; 1025–8192 medium; 8193–24576 high; above xhigh |
| enabled without budget | trimmed lowercase nonempty effort, otherwise auto |
| adaptive/auto | explicit effort, otherwise existing xhigh fallback; budget is not the source of intent |
| no thinking | explicit effort; otherwise absence |
| invalid budget (<-1, fractional, string, boolean, overflow) | reject before upstream send |

Sources: [`extractClaudeConfig`](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/thinking/apply.go#L740),
[`BudgetToLevel/LevelToBudget`](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/thinking/convert.go),
[`ValidateConfig`](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/thinking/validate.go).

| Target path/model | Normalization and final wire |
|---|---|
| Chat / Responses, unknown/custom model | existing extraction; `reasoning_effort` / `reasoning.effort`, including explicit none/auto; no inferred account support |
| Chat / Responses, frozen level-only Codex | budget bucket → nearest supported level (ties lower); none → low when zero unsupported; auto → medium when dynamic unsupported |
| Chat / Responses, frozen Claude budget-only | bound numeric source intent to frozen range before level conversion |
| Messages, frozen hybrid/adaptive Claude | explicit OpenAI level → supported adaptive `output_config.effort`; dynamic auto → adaptive without effort; zero uses declared disable type or lowest level |
| Messages, budget-only Claude / unknown custom | manual budget uses existing CPA effort table, model range when known, and remains strictly below caller output limit; too-small limits reject |
| known model with no thinking | compatible projection omits thinking; this does not invent support |
| same-protocol Passthrough | preserve valid original controls and JSON options; reject invalid known controls/ranges rather than claiming a native representation for compatibility-only 0/-1 |
| explicit LosslessBridge | retains its existing exactness/rejection contract |

CPAR deliberately retains its stricter output-limit invariant: it rejects an unrepresentable
manual budget rather than reproducing CPA's branch that can retain a budget at or above
max_tokens when max_tokens-1 is below the model minimum. Native standalone Messages effort
remains a native option; it does not synthesize thinking.

| Source → target | Tier behavior |
|---|---|
| Messages → Responses | exact `speed:"fast"` or trim/lower `service_tier:"fast"/"priority"` → `service_tier:"priority"` |
| Messages → Chat | omit speed/tier; fixed translator has no mapping |
| Responses → Messages | exact `service_tier:"priority"` → `speed:"fast"`; other selections omitted |
| Chat → Responses | trim/lower fast/priority → priority; ultrafast retained |
| other compatible direction | omit unmapped source tier |
| same-protocol native | original valid string values retained |

Non-string speed/tier is rejected. Unknown string selections are omitted only on compatible
cross-protocol paths. Requesting a tier is not a guarantee of priority service.
Sources: [Claude→Codex tier mapping](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/translator/codex/claude/codex_claude_request.go#L418),
[Responses→Claude](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/translator/claude/openai/responses/claude_openai-responses_request.go#L144),
[Chat→Codex](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/translator/codex/openai/chat-completions/codex_openai_request.go#L826).

Actual client #62/#64, cache placement/sampling/history-signature behavior, fixed services,
real Providers, remote CI, releases and deployment retain their independent qualification.

## Model names P1

Create a qualified name through the existing protected Alias resource:

```json
{"alias":"model","prefix":"team"}
```

POST this body to `/admin/public-models/{public_model_id}/aliases` with the existing
X-Config-Version, If-Match and management authentication. The returned/stored Alias is
`team/model`. Prefix whitespace and surrounding slashes are trimmed like CPA; the remaining
prefix is one case-sensitive ASCII alphanumeric/dot/underscore/hyphen segment (at most128 bytes).
The final Alias is at most256 UTF-8 bytes and has no whitespace/control characters. An explicit
prefix applies to this Alias only. Repeat for other names as needed. There is no global wildcard
prefix, force-prefix flag, implicit Provider selection, regex script or new permission.

List and delete the resulting exact Alias through existing endpoints. Draft validation,
publication and rollback use the same persisted version graph and immutable Snapshot. No schema
migration or existing-name rewrite is needed. `/v1/models` adds authorized qualified Alias names
to its existing exact-upstream-ID view; ordinary unqualified Aliases keep input-only discovery.
Management effective models include the same qualified names and actual candidate provenance.
Route Explain accepts a qualified Alias and its valid thinking suffix. Infinite suffix variants
are described by this grammar rather than enumerated as new models.

| Priority | Rule |
|---|---|
| 1 | Preserve existing exact upstream-ID resolution, then exact public name / exact Alias. A complete exact name containing parentheses is literal and body controls remain in force. |
| 2 | Only when no complete configured name exists globally, parse the last `(suffix)` and resolve its base through the same exact authorized path. A forbidden/ambiguous exact name cannot fall through to an accessible base. |
| 3 | Valid suffix overrides valid body thinking/effort; malformed body controls still reject. ADR-0126 disabled/budget/effort priority continues inside body extraction. |
| 4 | Normalize against the selected actual upstream model using P0. A name never constructs a new upstream ID, capability, credential entitlement or route grant. |

Numeric suffixes retain their numeric budget on budget-only/hybrid Messages models, bounded by
the frozen range and caller output limit; only level-only targets convert them to effort.
Same-family explicitly unsupported effort labels reject; documented cross-family conversion
can clamp to a supported level.

Accepted suffixes are `(none)`, `(auto)`, `(minimal)`, `(low)`, `(medium)`, `(high)`, `(xhigh)`,
`(max)`, and signed i64 numeric budgets >=-1 (0 off, -1 auto). Labels ignore ASCII case, while
model names and Alias/prefix matching retain exact case. Empty/unknown/whitespace-bearing labels,
budgets below-1, integer overflow and unresolved bases fail. Explicit LosslessBridge does not
admit new name controls. Known models without thinking reject suffix controls; unknown models
use the existing explicit fallback only when the candidate independently declares Reasoning.

Examples: `team/model(8192)`, `team/model(auto)`, `team/model(high)`. A literal exact Alias
`team/model(low)` wins as a complete name, so `{"reasoning":{"effort":"high"}}` keeps high on
that Alias. On a non-literal `team/model(512)`, the suffix wins; for frozen gpt-5.5 its minimal
budget normalizes to supported low. `team/model(0)` also becomes low for that model because the
frozen model does not support zero. Unknown/custom paths retain the documented none/auto fallback.

Inference JSON/SSE and count_tokens share resolution, exact-model constraints and Key/Group
permissions. Counting validates name-control syntax, Reasoning admission and known support without
constructing an inference request; it retains #68's counting algorithm and count-only transport.
Counts do not create inference events, Usage, ledger entries or quota observations. Count results
keep their original provenance and estimation limits; they do not predict a future generated bill.

Native inference with a suffix overlays only the normalized thinking/effort fields on the original
JSON body. Other valid native options remain. Without a suffix, native values remain unchanged.
Original bodies remain available to existing local diagnostics. Optional `CanonicalRequest.model_thinking`
retains the trusted name intent for stored continuation; absence is omitted during serialization,
so old payload bytes are unchanged. Older strict readers cannot read newly named payloads with
this field; do not claim binary rollback compatibility merely because schema39 is unchanged.

Reference differences are deliberate: CPA can parse suffixes even on an exact suffixed Alias,
accept unknown suffix content permissively, and use case-insensitive Alias lookup. CPAR retains its
existing literal exact names, case-sensitive routing and explicit failure for malformed/unknown
new names. Prefix spelling follows
[`applyModelPrefixes` / normalization](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/sdk/cliproxy/service_models.go#L705),
suffix spelling follows
[`ParseSuffix` and suffix parsers](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/thinking/suffix.go),
and suffix-before-body control intent follows
[`applyThinking`](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/thinking/apply.go#L317).

Tier mapping is installed only for the generic Responses and Anthropic Messages adapters with
the mappings above. Grok Build/Official/Console/Web and Kiro retain their existing adapter contracts;
unmapped tier hints are omitted on compatible projection. The frozen table currently covers
Claude/Codex entries; other model IDs retain fallback plus their existing adapter-specific guards.
