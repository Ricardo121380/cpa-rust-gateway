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
