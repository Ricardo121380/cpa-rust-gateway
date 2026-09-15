# CPAR Channel Onboarding Predeployment Acceptance

Date: 2026-09-16

## Scope

This record covers the channel-owned onboarding correction on commits
`88d6bc2ad068b824f1a6453407d8203e0ed5f902` and
`763b57053ce5e9478d71e77b8766da4fdeead625`. It is a predeployment record,
not a claim that an official Provider login has completed or that production
has switched releases.

## Reference-source comparison

| Reference | Reviewed revision | Behavior retained in CPAR |
| --- | --- | --- |
| CPA Manager Plus | `376b1e8c0fdab373552a5c73f2fe25908c6bd19c` | Channel-first account workspaces, direct per-account reauthorization, and separate provider maintenance. |
| CLIProxyAPI | `7bbfeaf8a7acf2cd5a834dcb0842539fe6aabc2b` | Kimi Coding device-code endpoints, Kimi token refresh material, explicit pending/slow-down/denied/expired terminal handling, and strict credential families. |
| CLI Proxy API Management Center | `c12997e1a544374336ea385d5e9a9bbabe1e4767` | Provider-specific OAuth cards, cancellable polling, independent auth-file import, and model/source management without conflating it with login. |

CPAR retains its own revisioned Draft, encrypted credential and same-origin
security model. It does not copy the references' browser storage, YAML editing,
secret export, or unbounded polling behavior.

## Delivered account flows

| Channel | First authorization | Import | Reauthorization / refresh |
| --- | --- | --- | --- |
| Codex / ChatGPT | Dedicated canonical Responses target, then OpenAI authorization-code journey | Existing Codex OAuth export, through the same owned target | Exact credential owner; legacy compatible owner accepted only after sealed Codex OAuth validation. |
| Claude | Dedicated canonical Messages target, then Claude authorization-code journey | Claude OAuth JSON or valid Claude API key | Exact credential owner with identity-preserving replacement; compatible legacy owner validated as Claude OAuth. |
| Kimi Coding | Dedicated device authorization at `auth.kimi.com`, automatic interval-aware polling | Strict `kimi_oauth` JSON via the owned Kimi target | Exact owner; encrypted access/refresh/device material is runtime-refreshable. |
| Kimi API | Explicit API setup only | API key on a separately owned `kimi` upstream | Never repurposed as Kimi Coding OAuth. |
| Kiro | AWS Builder ID / Identity Center device authorization; region and Start URL are advanced options | Kiro JSON or `ksk_` key, grouped by declared import region | Exact account owner preserves bindings and disabled state. |
| Grok Build | Existing device authorization entry | Grok Build export | Existing native reauthorization. |
| Grok Web / Console | No false first-login claim | Explicit SSO session import | Existing guarded native maintenance. |
| Generic API | No OAuth label | Explicitly chosen service, protocol and optional interface | API-key update remains distinct from OAuth refresh. |

## Correctness and interaction changes

- Named channels no longer enumerate or select compatible relays. Codex, Claude,
  Kimi and Kiro never render Provider or Endpoint controls in their onboarding
  journeys.
- First authorization prepares only the channel's canonical target inside one
  revision-bound Draft. It does not create a placeholder account, binding,
  route, or model grant.
- Imports for Codex, Claude, Kimi and Kiro resolve their owned target inside the
  same Draft, with Kiro's region read only from transient import metadata.
- Codex and Claude no-body target operations no longer receive an empty JSON
  body. Kiro alone sends its explicit region object.
- The OpenAPI `importChannelAccount` enum declares both distinct Kimi modes:
  `kimi-api` and `kimi-coding`; the generated Prism contract/client was produced
  by `sync-contract`.
- API provider choices, catalog endpoint/account choices, and the provider-card
  model entry all require an explicit operator selection. No `matches[0]`,
  `endpoints[0]`, or `targets[0]` fallback remains in Prism source.
- Kimi, Kiro and authorization dialogs discard transient sessions and cancel
  polling on close/session switch. Pending terminal replies retain the original
  non-secret challenge without overwriting a newer session.

## Automated evidence

The following completed against the candidate source:

- `npm --prefix web/prism run test`: 39 files, 320 tests.
- `npm --prefix web/prism run check` and production build.
- `node scripts/check-management-spa.mjs`: 152 generated operations and
  deterministic four-file double build.
- `cargo fmt --check` and
  `cargo clippy -p gateway -p gateway-http-actix -p provider-openai-compatible --all-targets -- -D warnings`.
- `p10_01_management_openapi_contract`: 14 tests, including the canonical Kimi
  import enum and secret write-only constraint.
- `p10_09_embedded_management_ui`: 3 tests.
- `managed_resource_inventory`: 38 HTTP regressions, including Codex, Claude
  and Kimi prepared imports with zero inferred endpoint bindings.
- Targeted Kimi runtime-credential and gateway refresh-worker regressions.

ChatGPT source review approved iterations 8 through 12 of
`c2c_auth_flow_01`; iteration 12 covered the last implicit provider-card
endpoint selection.

## Real local gateway and EgoLite evidence

An isolated state directory and loopback TLS mock started the actual embedded
gateway application. No production state, Provider credentials, DNS, Caddy or
Autoreg configuration was used.

- First-login bootstrap and password-change flow were verified on the isolated
  administrator account; browser sessions remained memory-only after reload.
- Codex, Claude, Kimi and Kiro onboarding were inspected in the real React
  application. Each showed authorization without a Provider or Endpoint field.
- Kimi Coding showed both device authorization and strict OAuth JSON import.
- An actual Draft-only Kimi import prepared `kimi-coding`, persisted an
  `oauth_json` credential, returned no secret, created zero bindings, and
  remained unpublished.
- A loopback Responses request returned a completed result and was observed in
  protected request history. The local mock recorded zero non-mock Provider
  inference calls.
- All 14 Prism routes loaded without a horizontal overflow at 1440x900,
  1280x720 and 390x844. The account sheet opened with focus inside the dialog,
  trapped Tab navigation, and closed through Escape before any edit. Dark and
  light themes were both rendered.
- The current embedded provider-card action was reloaded after clearing the
  test browser asset cache and verified as `#/models?add=model`, without a
  `from_endpoint` seed.

## Release status and remaining human validation

The existing production release remains
`0ff3b82e715ce750f05a7116b444f375edfd60dd` at this point. Its read-only
Oracle preflight found the CPAR service, Caddy and Autoreg active; loopback
listeners and `/healthz` were healthy. The compatible rollback binary is
retained in the existing release directory.

The signed artifact workflow for `763b57053ce5e9478d71e77b8766da4fdeead625`
has been started. Production installation, public readback and the final
delivery receipt remain pending its signed ARM64 artifact.

Official Provider consent cannot be supplied by a synthetic test. A human must
complete the relevant consent pages for any live Codex/ChatGPT, Claude, Kimi
Coding, Kiro Builder ID or Grok Build account that should be verified against a
real Provider. Those steps must be reported separately from local mock
evidence.
