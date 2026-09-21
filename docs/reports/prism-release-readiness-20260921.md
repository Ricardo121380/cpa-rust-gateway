# Prism release readiness — 2026-09-21

Candidate implementation: `d232f9416157cbc4ff3a7f0788307903bc93b800`.
Status: local final gates passed after a test-fixture correction. Superseding release `a190767` passed formal gates, signing, isolated rollback and Oracle deployment; see [production delivery](prism-workspace-production-20260921.md).

## Channel evidence boundary

| Channel | Evidence retained | Official consent boundary |
| --- | --- | --- |
| Grok Build | Historical real Chrome first device authorization and same-account reauthorization, recorded in [channel release](prism-channel-production-release-20260911.md). | This is historical success, not proof every current grant remains valid. No new live grant/inference tested this round. |
| Codex / ChatGPT | Canonical Responses target, strict OAuth import, exact-owner replacement and local protocol regressions in [onboarding delivery](cpar-channel-onboarding-delivery-20260916.md). | New official login/consent not completed in this round. |
| Claude | Canonical Messages target, OAuth/API import, identity-preserving replacement, same delivery record. | New official login/consent not completed in this round. |
| Kimi Coding | Device authorization flow, bounded polling, strict encrypted OAuth import; real disposable gateway draft-only import in the same delivery record. | Synthetic import/device exchange does not prove official consent. |
| Kiro | Real AWS device-start/pending/cancel and injected exchange regressions in [alignment delivery](prism-complete-alignment-delivery-20260915.md). | Successful official exchange not claimed. |
| Grok Web / Console | Explicit SSO import and native maintenance; no fabricated OAuth first-login flow. | Existing SSO metadata is not fresh provider-validity evidence. |
| Kimi API / generic API | Explicit key/protocol setup and loopback mock validation. | OAuth consent is not applicable; real inference deliberately excluded. |

The current scoped plan requires preserving these distinctions, not inventing new official logins to certify a UI release. Human consent remains necessary whenever an operator chooses to validate a live grant. No channel is relabeled as fully live-verified on the strength of a fixture.

## Candidate scope and rollback preparation

Read-only Oracle preflight on this date: CPAR active; current release pointer resolves to `763b57053ce5e9478d71e77b8766da4fdeead625`; loopback health passed. This verifies process health only.

Backend diff from that production revision is limited to billing catalog capacity/read behavior, atomic store enforcement and corresponding contract/tests. No migration or OAuth-provider source changes appear in that diff. Frontend includes the documented workspaces, pending-application lifecycle, visual refinement and acceptance fixes.

Retain the installed signed `763b570` binary as the prospective fallback. Before cutover, independently verify its hash and candidate compatibility against a fresh isolated production copy; retain current rotating credentials and administrator/history stores. No stale database restore, schema downgrade, DNS/Caddy/firewall/Autoreg change or real Provider inference is authorized by this preparation. Rehearsal and signature verification are still required; matching schema alone is not rollback proof.

## Gate record

- Frontend units: 48 files / 390 tests passed for the candidate.
- Initial local `scripts/check.sh fast` stopped at two legacy OAuth HTTP fixtures: the sealed credential did not identify a Codex OAuth family, so strict admission correctly returned 400. Updated only synthetic test credentials; production admission is unchanged. Both affected tests and the managed-resource suite passed (42 tests). The corrected full gate **passed**, including workspace/all-features Rust tests, Clippy, embedded SPA, serve envelope, contract, source/boundary, document and secret checks, with receipt `/tmp/prism-release-gate-final-20260921.md` and log `/tmp/prism-release-gate-final-20260921.log`.
- Frozen Chromium full suite: **291 passed**, 6.4 minutes, zero retries; `/tmp/prism-release-e2e-20260921.log`.
- Local supply-chain gate passed (pinned tool versions, dependency policy and RustSec audit); `/tmp/prism-supply-chain-20260921.md`.
- Remote formal gate: [35571436956](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/35571436956).
- Signed artifact workflow: [35571439717](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/35571439717). Both were dispatched against exact candidate `d232f94` and then **cancelled** after the local Rust gate failure. Neither is accepted release evidence; a new committed candidate and new workflows are required.

Earlier bounded UI, real mock chain and performance evidence remains indexed by [integrated acceptance](prism-integration-acceptance-20260921.md), [keyboard acceptance](prism-keyboard-readiness-20260921.md), [operational states](prism-operational-state-readiness-20260921.md), and [dashboard/maintenance](prism-dashboard-maintenance-readiness-20260921.md).
