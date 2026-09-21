# Prism workspace release — 2026-09-21

**Acceptance correction:** A subsequent user-reported Safari crash invalidated the broad frontend completion claim. Health, signature and data-preservation checks remain valid, but did not cover mixed-release browser modules. See [incident and hotfix](prism-module-url-hotfix-20260921.md); do not interpret the original release checks as exhaustive production browser acceptance.

## Release status

Production now serves signed `a19076747c94324f8acb32b70c44410f2aa62eb3` on the existing Oracle service and domain. M4 integrated acceptance and M5 deployment are complete for the scoped workspace plan; official consent boundaries below remain explicit.

## Delivered scope

- Accounts: preserve channel-specific onboarding and real identities; prevent delayed search from interrupting an open editor and protect writes, cancellation and session changes.
- Providers: shared inline creation and editing, explicit pending-application results, accurate connection/protocol names and fresh draft reads.
- Models: inline route/candidate maintenance with exact model IDs, source validation, revision checks and non-replayed uncertain outcomes.
- API keys: explicit model selection, private access-group lifecycle, one-time secret handling and unsupported-limit restrictions.
- Requests and usage: terminal/request/attempt distinctions, unknown values, filters, diagnostic links and correct null access-group labels.
- Billing: complete 512-row edit/preview pagination, frozen submit content, global catalog versus pending policy results, and atomic 256-catalog capacity enforcement.
- Settings and advanced maintenance: inline egress editing, pending configuration application, version differences, audit/backup preflight and honest draft runtime availability.
- Shared presentation: approved OpenDesign K3 high adaptation, responsive glass materials, stable action areas, busy/dirty navigation protection and restored keyboard focus. Dashboard failures now expose retry instead of an indefinite loading placeholder.

## Acceptance evidence

[Local release gates](prism-release-readiness-20260921.md): 390 frontend unit tests, 291 Chromium tests, 1,282 Rust tests passed (9 ignored), full fast gate and supply-chain gate passed. The OAuth fixture correction changes test data only; strict production admission remains unchanged.

Real embedded gateway/loopback mock, three viewport sizes, preferences, keyboard, error states and performance evidence is recorded in [integration acceptance](prism-integration-acceptance-20260921.md), [keyboard readiness](prism-keyboard-readiness-20260921.md), [operational states](prism-operational-state-readiness-20260921.md) and [dashboard/maintenance readiness](prism-dashboard-maintenance-readiness-20260921.md).

- Exact-revision formal gate: [35573470369](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/35573470369), passed (Fast, supply-chain and required delivery jobs).
- Both native signed targets passed [35573473533](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/35573473533).
- ARM64 artifact structure, revision, manifest, SBOM and signed receipt passed the repository verifier; independent Cosign verification returned `Verified OK` for the repository release workflow identity.
- ARM64 binary SHA-256: `271e6d229ac9615dab739ef521a7b8b2e3c3022b887cc0c681cad39bf2a05555`.
- Network-isolated fresh production-copy fallback → candidate → fallback → candidate passed. Schema stayed 28; effective permissions, administrator store, seven accounts, 2,229 historical events and 593 ledger entries were preserved. Temporary copied state and credentials were removed after the rehearsal.

## Rollback and boundaries

Compatible signed fallback: `763b57053ce5e9478d71e77b8766da4fdeead625`, independently hash-checked against its previous release receipt. Preserve the latest state and rotating credentials; switch binaries only. Never restore stale administrator/token databases. Reassess rollback compatibility after later configuration or code changes.

No new real Provider inference, account consent, production cleanup, DNS/Caddy/firewall or Autoreg changes are part of this release. [Channel evidence boundaries](prism-release-readiness-20260921.md#channel-evidence-boundary) distinguish historical official success, injected protocol evidence and manual validation still requiring the account owner. Codex with ChatGPT was not used in this round; no new C2C approval is claimed.

## Production readback

- Atomic cutover and readiness passed in 1,023 ms on the successful attempt; systemd runs the exact verified ARM64 binary, schema28.
- Public health, all four deterministic frontend hashes and CSP passed. Public `/admin/system`, `/admin/accounts/inventory` and `/admin/config-versions` remain HTTP404; both gateway listeners remain loopback-only.
- Active configuration and effective permissions stayed unchanged. Existing administrator store, account/resource identities, 2,229 historical events and 593 ledger entries were retained; no database restoration or cleanup occurred.
- EgoLite rendered the public administrator login at 1440×900, 1280×720 and 390×844 without horizontal overflow. No production password was entered; authenticated workflow acceptance remains the documented local real-gateway evidence and host-local protected readback, not a claim of a new public login.
- The first cutover encountered systemd `203/EXEC`: the release directory inherited mode0700 from the deployment script umask. Automatic binary rollback succeeded without restoring data. The script now explicitly sets the binary-only release directory to0755 and tests execution as the actual service user; the second cutover passed. Secret/state/backup permissions were not relaxed. Both attempt receipts are retained privately.

Human acceptance: [Prism administrator login](https://cpar.142857142.xyz/admin-ui/#/unlock), using the existing administrator account. Refresh any tab that was open before deployment. Suggested checks: account authorization workspace, provider editing, model/candidate editing, key permissions, price preview and pending-application review.
