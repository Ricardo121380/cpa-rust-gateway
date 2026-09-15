# CPAR Channel Onboarding Delivery

Date: 2026-09-16

Production revision: `763b57053ce5e9478d71e77b8766da4fdeead625`

Rollback revision: `0ff3b82e715ce750f05a7116b444f375edfd60dd`

## Scope and reference comparison

This delivery corrects channel-owned account onboarding, import,
reauthorization, credential refresh and binding behavior in Prism. It covers
`88d6bc2ad068b824f1a6453407d8203e0ed5f902` and
`763b57053ce5e9478d71e77b8766da4fdeead625`.

| Reference source | Reviewed revision | CPAR alignment |
| --- | --- | --- |
| CPA Manager Plus | `376b1e8c0fdab373552a5c73f2fe25908c6bd19c` | Channel-first account workspaces, per-account reauthorization, and provider maintenance separated from normal onboarding. |
| CLIProxyAPI | `7bbfeaf8a7acf2cd5a834dcb0842539fe6aabc2b` | Kimi Coding device-code and refresh material, bounded polling terminal states, and credential-family validation. |
| CLI Proxy API Management Center | `c12997e1a544374336ea385d5e9a9bbabe1e4767` | Provider-specific authorization cards, cancellable polling, independent authorization-file import, and source/model maintenance outside the login flow. |

CPAR retains its revision-bound Draft ownership, CAS writes, encrypted secret
storage and same-origin management model. It does not adopt reference-project
browser secret storage, YAML editing or secret export.

## Delivered account flows

| Channel | First authorization | Import | Reauthorization / refresh |
| --- | --- | --- | --- |
| Codex / ChatGPT | Canonical Responses target and OpenAI authorization-code journey | Existing Codex OAuth export through its owned target | Exact credential owner; compatible legacy data is accepted only after sealed Codex OAuth validation. |
| Claude | Dedicated Messages target and Claude authorization journey | Claude OAuth JSON or valid Claude API key | Exact owner replacement with identity checks. |
| Kimi Coding | Kimi device authorization at `auth.kimi.com`, interval-aware automatic polling | Strict `kimi_oauth` JSON through the owned Kimi target | Runtime-refreshable encrypted access, refresh and device material. |
| Kimi API | Explicit API configuration only | API key on a distinct owned `kimi` upstream | Never presented as Kimi Coding OAuth. |
| Kiro | AWS Builder ID / Identity Center device journey; region and Start URL are advanced options | Kiro JSON or `ksk_` key with declared region | Exact account owner keeps bindings and disabled state. |
| Grok Build | Existing device authorization | Grok Build export | Existing native reauthorization. |
| Grok Web / Console | No false first-login claim | Explicit SSO-session import | Existing guarded native maintenance. |
| Generic API | No OAuth label | Explicit service, protocol, optional interface and API key | API-key update remains separate from OAuth refresh. |

Named channel dialogs no longer render a Provider or Endpoint selector. Their
canonical target is prepared only inside the selected, revision-bound Draft;
it creates no placeholder account, binding, route or model grant. API setup,
catalog source selection and provider maintenance still require an explicit
operator choice where those choices are meaningful.

## Related defects removed

- Codex, Claude, Kimi and Kiro no longer choose a compatible relay through
  display name, protocol compatibility or array order.
- Kimi Coding and Kimi API are distinct contract values; the authoritative
  OpenAPI was updated before Prism generated client synchronization.
- Provider-card model activation requires an explicit endpoint, then an
  explicit account. No `matches[0]`, `endpoints[0]` or `targets[0]` selection
  remains in Prism source.
- Codex and Claude no-body target actions no longer receive a synthetic empty
  JSON body. Kiro alone transmits the required explicit region object.
- Account capability projection uses the concrete account and sealed credential
  family for first authorization, import, replacement and refresh actions.
- Dialog close, channel change and expired sessions cancel polling and remove
  transient secret input; stale poll replies cannot overwrite a newer session.

## Verification evidence

### Source, contract and regression checks

- `npm --prefix web/prism run test`: 39 files, 320 tests passed.
- `npm --prefix web/prism run check`, production build, and
  `npm --prefix web/prism run check:full` passed; `check:full` reported
  `check-prism-spa: OK`.
- `node scripts/check-management-spa.mjs` passed with 152 generated operations
  and the deterministic four-file double-build check.
- `cargo fmt --check` and
  `cargo clippy -p gateway -p gateway-http-actix -p provider-openai-compatible --all-targets -- -D warnings` passed.
- `p10_01_management_openapi_contract` (14 tests),
  `p10_09_embedded_management_ui` (3 tests),
  `managed_resource_inventory` (38 tests), targeted Kimi credential/runtime
  tests and credential refresh regressions passed.
- ChatGPT review task `c2c_auth_flow_01` approved iterations 8 through 12.
  Iteration 12 specifically reviewed the last implicit provider-card endpoint
  fallback.

### Local real-gateway and EgoLite acceptance

An isolated loopback gateway ran the actual embedded Prism application against
a temporary state directory and protocol-level mock. It did not use production
state or invoke a real Provider.

- Codex, Claude, Kimi and Kiro onboarding showed no Provider or Endpoint field.
- Kimi Coding showed both device authorization and strict OAuth JSON import.
- A Draft-only Kimi import persisted an `oauth_json` credential, returned no
  secret, created zero bindings and remained unpublished.
- A loopback Responses request completed and appeared in protected request
  history; the mock recorded zero non-mock Provider inference calls.
- All 14 Prism routes loaded without horizontal overflow at 1440x900, 1280x720
  and 390x844. Light and dark themes, dialog focus containment, Tab traversal
  and Escape close behavior were checked.

### Signed release and production readback

- GitHub Actions signed artifact build:
  [run 35004340065](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/35004340065),
  both native targets successful for revision `763b570…`.
- The downloaded ARM64 artifact passed the repository verifier with required
  signature and receipt, and independent `cosign verify-blob` returned
  `Verified OK` against the repository release workflow identity.
- Installed binary SHA-256:
  `72900bedfcd784ddcd9f8d5ad40ca0aecae7bbe0e74c16dbf2a74d2b01505973`.
- Before cutover, CPAR created a root-owned SQLite online backup and current
  binary receipt, then started the signed candidate against an isolated copy
  of the production database and credential files. Candidate health and SQLite
  `PRAGMA quick_check` passed; the temporary candidate state and credentials
  were deleted after the preflight.
- The release pointer was atomically switched, the systemd service restarted,
  and automatic rollback was armed for any failure. Post-cutover checks passed:
  service active, current binary hash exact, loopback `/healthz`, public
  `/healthz`, and production SQLite `PRAGMA quick_check`.
- The public login UI at
  [https://cpar.142857142.xyz/admin-ui/#/unlock](https://cpar.142857142.xyz/admin-ui/#/unlock)
  rendered in EgoLite at 1440x900 without horizontal overflow. The page showed
  only the administrator account/password form; no credential was entered.
- The public `index.html`, `assets/main.js`, `assets/vendor.js` and
  `assets/index.css` all returned HTTP 200 and exactly matched the local
  deterministic build hashes. CSP is present and disallows `unsafe-inline` and
  `unsafe-eval`. Public `/admin/system`, `/admin/accounts/inventory` and
  `/admin/config-versions` each returned 404.

No DNS, Caddy, firewall, Autoreg, administrator store, production account,
historical request or billing-ledger data was changed. The prior release
directory remains the rollback target.

## Human validation boundary

The automated suite uses protocol-level mocks by design. For a live channel,
the account owner must still complete any official consent, second-factor or
device-code confirmation for Codex/ChatGPT, Claude, Kimi Coding, Kiro Builder
ID or Grok Build. Those real-provider actions are intentionally not claimed by
this delivery and must be recorded separately from the local evidence above.
