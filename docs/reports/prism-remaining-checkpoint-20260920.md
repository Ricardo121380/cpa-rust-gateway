# Remaining-plan functional checkpoint — 2026-09-20

Status: implementation in progress, not milestone completion or deployment approval. Base: `75715595b381e234564b949ee390cafb4553c701`; billing checkpoint committed as `503b740`; plan/provenance committed as `990855a`. Subsequent configuration lifecycle work remains uncommitted.

## Implemented so far

- Billing import now uses a labelled inline region with edit, frozen preview, saving and receipt phases. Existing ownership/CAS and exact-target receipts remain intact. `OperationBoundary` coordinates local replacement/dock admission; `InlineWorkspace` owns the router blocker; confirmation Sheets explicitly inherit navigation ownership without registering another blocker.
- Dirty departure asks once and restores the edited field. Busy writes reject Close, Escape, Back and dock admission without replaying navigation after completion. Ordinary standalone Sheets retain their original cleanup behavior.
- Price editing, full previews, diff previews, expanded catalog rows and inspectors display bounded 50-row windows. All data remains in the backing model. Off-page invalid rows are located and focused. Six rates, zero/missing distinctions, raw model IDs and full frozen payloads survive pagination and mode changes.
- Global imports/restores no longer auto-publish configuration. A deferred configuration-task mode preserves the existing default for unrelated callers. Active-context policy changes create an admission draft only on submit. The pending draft is retained in session memory, survives viewing other configurations, and is reset with the session. Subsequent key edits reuse that draft; publication remains explicit. Viewing another context requires explicit pending-draft resumption rather than silently redirecting a captured edit.
- Access groups no longer accept newly invented limits. Historical nonempty limits require explicit clearing or disabled preservation; no silent deletion. Legacy Access/Versions/proxy Sheets receive busy protection. Further full lifecycle migration of those legacy editors remains pending.
- Monitoring regressions are being updated from obsolete “no request metrics exist” assumptions to real request metrics, separate attempts, ledger and failure scopes. Runtime deep-link target display now uses existing resource identity components.

## Fresh evidence

- TypeScript passed.
- Frontend unit suite: 46 files / 371 tests passed.
- Focused integrated Chromium suite: 54 passed (billing, inline ownership, access groups, modal foundation, key permissions and publication).
- Embedded gateway Rust tests: 3 passed.
- `check:full` and `node scripts/check-management-spa.mjs`: passed; 152 generated operations and four-file reproducible output.
- Price boundaries: 1/50/51/512 entries; 513 rejection; 1,024 disjoint differences; off-page row 51 validation; payload readback.
- Cross-workspace fixture sequence: active → policy admission draft → key permission edit in the same draft → explicit publication. One fork, zero early publishes, one final publish.
- Isolated real embedded gateway/EgoLite: appended a future-effective synthetic catalog; durable receipt says globally saved and not published, then selected the exact draft. No real Provider inference. Three viewports (1440×900, 1280×720, 390×844) showed no document horizontal overflow, zero modal wrappers for the editor, and 44px footer controls. Screenshots are intermediate functional evidence, not final visual approval.
- Local evidence logs: `/tmp/prism-grill-checkpoint-e2e-20260920.log`, `/tmp/prism-inline-rust-20260920.log`; screenshots `/tmp/prism-inline-real-{1440,1280,390}.png`. New later changes require their own verification.

## OpenDesign status and adoption constraints

MCP used existing project `prism-gateway-console-redesign-a735`, agent Pi, requested `cc-switch-kimi-for-coding/k3:max`.

- First run `e1f2cd06-ba33-48ff-bcbe-ecfde8d3589d`: no artifact, not accepted.
- Second run `25c53857-b298-4a28-9046-fb5bc6f21042`: new HTML/spec fetched through MCP and archived at `docs/design/prism-workspace-20260920/`. OpenDesign reports `entry_not_touched` because the existing root entry was intentionally preserved. Files exist in the requested subdirectory.
- Actual Pi session metadata for both runs resolves provider `cc-switch-kimi-for-coding`, model `k3`, thinking `high`. Installed Pi capability code clamps unsupported max to a supported level. User explicitly approved refining the existing high design; no max-compliance claim.
- Prototype behavior is not authoritative: invented price multipliers, user-entered authorization identity, applying on Escape, immediate key issuance and horizontal mobile navigation conflict with the confirmed production plan. These are explicitly rejected for production adoption. Only reviewed visual/layout proposals may transfer; no prototype script is copied to the SPA.

## Still required

C2C blocking review/approval; pending-batch human-readable combined change review and complete resumption edge cases; performance baseline (five runs); current real-gateway cross-workspace publication and expanded responsive/accessibility checks; remaining Provider/Access/egress editor lifecycle migrations and other workspace refinement; final integration gates, signed deployment and delivery report. The whole plan is not complete and production has not changed.

Follow-up verification: monitoring suite 9/9 passed after updating tab navigation/history-resource selection and the Runtime target label. The inline heading now scrolls into view on phase changes; billing editor is placed before catalog inventory so it does not open below a long list. Performance comparison is being collected using the same synthetic fixture data on an isolated HEAD frontend and the working frontend, each with five warm runs; this does not replace real-gateway acceptance.

## Intermediate review corrections (B12–B14)

C2C requested three correctness fixes; all are now implemented, awaiting follow-up review:

- B12: identical router destinations are a deliberate no-op. Accepted same-component query navigation retires the inline action after the router changes location; it does not manually rebuild history or prematurely remove the blocker. Six inline regressions passed, including the new query/current-tab case.
- B13: active and existing-draft policy branches share pending-batch admission. Both bind and clear refuse another draft before writing; the pending identity is retained. Targeted billing/configuration unit suite: 53 passed.
- B14: management catalog insertion checks the shared 256-item capacity in the same immediate SQLite transaction as the revision/audit mutation. HTTP returns a documented 400 Error without mutation. Store concurrency, service import/restore rollback and HTTP readable-capacity tests passed. Fixture and browser capacity behavior now match; billing lifecycle suite: 15 passed. No deletion or schema migration. Narrow CR: `docs/change-requests/CR-billing-catalog-capacity-20260920.md`.
- Affected Rust Clippy (`gateway-store`, `gateway-control`, `gateway-http-actix`, all targets, warnings denied) passed.

Performance results are recorded in `prism-remaining-performance-20260920.json`: five warm local-development medians, same 512 synthetic rows. Startup 129.4 → 133.9 ms; editor paint 1802.1 → 858.6 ms; request-filter paint 1580.8 → 842.1 ms. Editor mounts 512 → 50 rows; measured operation invocation counts remain 1 for opening and 5 for filtering. No measured regression crosses both 10% and 50 ms. These noisy dev/frame-scheduling figures are not production response-time claims; final visual changes still require their own comparison.

C2C follow-up: iteration 9 returned `APPROVED / B12_B13_B14_CORRECTIONS`, explicitly limited to the intermediate corrective checkpoint. It independently read execution outputs 39–44 (6 inline cases, Store/service capacity, 3 billing HTTP, 15 lifecycle cases, 373 units/double build). Whole-plan acceptance and visual adoption are not approved. The final gateway binary build and three embedded UI tests also passed after the capacity repair.

## Current state after user steering

The configuration lifecycle host, inline pending review, explicit draft adoption and fixed-ID draft creation are implemented in the working tree, not yet accepted as a complete checkpoint. Recent focused evidence: 19 lifecycle/store unit tests and four browser cases passed. Remaining: migrated publication/diff regressions, wider ownership recovery checks, real gateway apply/rollback verification, advanced editor migration, production visual adaptation and whole-app acceptance. This turn does not use Codex with ChatGPT, as explicitly requested by the user; historical approvals do not cover these new changes. No production deployment occurred.

### Direct local integration verification

Fresh current-code verification: TypeScript and 390 unit tests passed; 5 publication lifecycle cases, 1 concurrent diff pagination/restart case and 10 inline ownership/pending/edit cases passed. Four-file deterministic build / SPA gate and 3 embedded gateway Rust tests passed. Comparison restart was corrected to refresh revision metadata; draft adoption rejects a mismatched returned identity. These are fixture and embedded-asset checks, not real-gateway lifecycle acceptance. Remaining advanced editors, visual rollout, real-gateway acceptance and deployment remain open.
