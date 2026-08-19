# Git branch inventory — 2026-08-19

This is a point-in-time, value-free inventory of every local/remote branch visible during the
public-main integration audit. It records merge topology; it does not delete, rebase or merge any
branch.

The checkout had **59 local branch names and the same 59 branch names under `origin`**. Fifty-eight
name pairs had identical tips. The sole local/remote tip divergence was
`codex/p13-10-websocket`; its remote tip appears in the table and its local successor is recorded
in the dedicated section below. The symbolic remote default ref is not a separate branch.

The 59 remote heads and the five integration-critical tips (`main`, current integration, P13-08,
P13-10 and P13-11) were revalidated with `git ls-remote --heads origin` on 2026-08-20 and were
unchanged.

## Executive conclusion

- `main` is still `cdfff7a`; the implemented backend is not missing from GitHub—it is on feature
  branches and immutable phase tags.
- `origin/codex/p13-11-egress` is the latest formally gated backend line at `1bb1fd7` and is fully
  contained by the current integration candidate.
- `claude/route-candidates` at `cd27ff3` is the current integration candidate: 81 commits ahead and
  0 behind `origin/main`, and synchronized with its remote before the current uncommitted work.
- Forty-nine historical branch tips are already ancestors of `main`; merging them again would do
  nothing.
- Most later P13 tips are not in `main` but are already ancestors of the integration candidate;
  merge the integration candidate once, not each phase branch separately.
- `codex/p13-08-channel-pin-gate` is the one non-ancestor phase tip requiring explicit
  reconciliation. Its one-line functional correction is patch-equivalent to later work; its two
  remaining commits are historical closeout evidence. Do not blindly merge the old branch.
- The local `codex/p13-10-websocket` has seven commits beyond its remote tip, all already present in
  the integration candidate. Do not force-update an old phase branch just to make the tips match.
- The repository is public and `main` is the default branch, but the audit found no open PR, no
  repository ruleset and no classic `main` branch protection. Protect `main` and require the
  reviewed CI/Gate checks before merging the integration PR.

## Why GitHub did not show a completed integrated backend on `main`

1. The backend commits and formal tags were pushed to phase branches.
2. The default branch never received the P13 integration PR.
3. `.github/workflows/ci.yml` runs on `main` pushes and pull requests, not arbitrary feature-branch
   pushes.
4. The formal delivery workflow is opt-in through immutable phase tags, manual dispatch or a PR
   closeout label.
5. Therefore a synchronized feature branch can legitimately have no commit status/check run.

The safe path is: finish and review current Prism work, reconcile P13-08 explicitly, run local
gates, protect `main`, push the integration tip, open one PR into `main`, run lightweight PR CI,
then run one formal gate for the immutable final revision.

## Complete branch table

`Main contains` means the branch tip is an ancestor of `origin/main`. `Integration contains` means
the tip is an ancestor of `claude/route-candidates`. Ahead/behind values are relative to
`origin/main` at the audit time.

| Branch | Tip | Main contains | Integration contains | Ahead/behind main | Recommendation |
|---|---:|:---:|:---:|---:|---|
| `claude/merge-prism-frontend` | `4bcdd3a` | No | Yes | `22/0` | Covered by integration; archive after integration merge. |
| `claude/route-candidates` | `cd27ff3` | No | Yes | `81/0` | Active integration candidate; review, gate and PR to `main`. |
| `codex/g0-phase-gate` | `11bc87a` | Yes | Yes | `0/456` | Already merged; retain for audit window, then archive. |
| `codex/g1-phase-gate` | `8fb9ff3` | Yes | Yes | `0/441` | Already merged; retain for audit window, then archive. |
| `codex/p0-01-repo-baseline` | `b677113` | Yes | Yes | `0/465` | Already merged; archive after audit window. |
| `codex/p0-02-doc-traceability` | `1729156` | Yes | Yes | `0/464` | Already merged; archive after audit window. |
| `codex/p0-03-rust-workspace` | `157ee62` | Yes | Yes | `0/463` | Already merged; archive after audit window. |
| `codex/p0-04-quality-gates` | `5858b07` | Yes | Yes | `0/462` | Already merged; archive after audit window. |
| `codex/p0-05-ci-baseline` | `6a77728` | Yes | Yes | `0/460` | Already merged; archive after audit window. |
| `codex/p0-06-environment-baseline` | `a8e1c91` | Yes | Yes | `0/459` | Already merged; archive after audit window. |
| `codex/p1-01-request-context-errors` | `1c59b1e` | Yes | Yes | `0/454` | Already merged; archive after audit window. |
| `codex/p1-02-canonical-request` | `c348b80` | Yes | Yes | `0/453` | Already merged; archive after audit window. |
| `codex/p1-03-canonical-event` | `fbda3a8` | Yes | Yes | `0/452` | Already merged; archive after audit window. |
| `codex/p1-04-bounded-stream` | `2283677` | Yes | Yes | `0/451` | Already merged; archive after audit window. |
| `codex/p1-05-openai-responses-adapter` | `edc0bbf` | Yes | Yes | `0/450` | Already merged; archive after audit window. |
| `codex/p1-06-deterministic-mock-provider` | `836e95d` | Yes | Yes | `0/449` | Already merged; archive after audit window. |
| `codex/p1-07-actix-responses-handler` | `a3095e3` | Yes | Yes | `0/447` | Already merged; archive after audit window. |
| `codex/p1-08-client-key-auth` | `43f51ad` | Yes | Yes | `0/445` | Already merged; archive after audit window. |
| `codex/p1-09-tool-chunk-properties` | `d4c4698` | Yes | Yes | `0/443` | Already merged; archive after audit window. |
| `codex/p10-control-plane` | `df459a6` | Yes | Yes | `0/267` | Already merged; archive after audit window. |
| `codex/p11-release-hardening` | `0f64aaf` | Yes | Yes | `0/248` | Already merged; archive after audit window. |
| `codex/p12-deployment` | `c02a689` | Yes | Yes | `0/4` | Already merged; archive after audit window. |
| `codex/p13-05-billing-ledger` | `da96a8c` | No | Yes | `4/1` | Covered by integration; do not merge separately. |
| `codex/p13-06-account-pool` | `b80493f` | No | Yes | `5/1` | Covered by integration; do not merge separately. |
| `codex/p13-06b-provider-adapters` | `e34b3cb` | No | Yes | `29/0` | Covered by integration; do not merge separately. |
| `codex/p13-06c-operator-feedback` | `b6a7085` | No | Yes | `39/0` | Covered by integration; do not merge separately. |
| `codex/p13-08-channel-pin-gate` | `b9d0a89` | No | No | `42/0` | Explicit reconciliation; never blind-merge. |
| `codex/p13-09-stored-responses` | `8d7c4f6` | No | Yes | `45/0` | Covered by integration; do not merge separately. |
| `codex/p13-10-websocket` (remote) | `3dc5dc5` | No | Yes | `49/0` | Remote historical tip; local successor is already covered. |
| `codex/p13-11-egress` | `1bb1fd7` | No | Yes | `78/0` | Latest gated backend, covered by integration. |
| `codex/p13-management-operations` | `ca8957c` | Yes | Yes | `0/1` | Already merged; archive after audit window. |
| `codex/p2-01-control-plane-schema` | `b276531` | Yes | Yes | `0/439` | Already merged; archive after audit window. |
| `codex/p2-02-routing-access-schema` | `de7cee4` | Yes | Yes | `0/437` | Already merged; archive after audit window. |
| `codex/p2-03-aead-secret-store` | `e97a887` | Yes | Yes | `0/435` | Already merged; archive after audit window. |
| `codex/p2-04-client-key-hmac` | `3c5a1a1` | Yes | Yes | `0/433` | Already merged; archive after audit window. |
| `codex/p2-05-control-plane-service` | `5aff6bf` | Yes | Yes | `0/431` | Already merged; archive after audit window. |
| `codex/p2-06-route-compiler` | `6914540` | Yes | Yes | `0/429` | Already merged; archive after audit window. |
| `codex/p2-07-route-snapshot` | `28b7102` | Yes | Yes | `0/427` | Already merged; archive after audit window. |
| `codex/p2-08-snapshot-auth` | `a29163b` | Yes | Yes | `0/425` | Already merged; archive after audit window. |
| `codex/p2-09-egress-policy` | `c21d5b5` | Yes | Yes | `0/423` | Already merged; archive after audit window. |
| `codex/p2-10-management-api-cli` | `5770e5d` | Yes | Yes | `0/419` | Already merged; archive after audit window. |
| `codex/p3-01-openai-responses-request` | `95bcd6d` | Yes | Yes | `0/416` | Already merged; archive after audit window. |
| `codex/p3-02-upstream-client-pool` | `8b79bc6` | Yes | Yes | `0/413` | Already merged; archive after audit window. |
| `codex/p3-03-priority-scheduler` | `f954770` | Yes | Yes | `0/410` | Already merged; archive after audit window. |
| `codex/p3-04-credential-pool` | `b1c2083` | Yes | Yes | `0/407` | Already merged; archive after audit window. |
| `codex/p3-05-runtime-health` | `cc10a67` | Yes | Yes | `0/404` | Already merged; archive after audit window. |
| `codex/p3-06-attempt-orchestrator` | `68dca07` | Yes | Yes | `0/401` | Already merged; archive after audit window. |
| `codex/p3-07-models-response-rewrite` | `9fdf1a6` | Yes | Yes | `0/398` | Already merged; archive after audit window. |
| `codex/p3-08-structured-events` | `84f48f9` | Yes | Yes | `0/395` | Already merged; archive after audit window. |
| `codex/p3-09-mock-upstream-e2e` | `a9fb25d` | Yes | Yes | `0/392` | Already merged; archive after audit window. |
| `codex/p3-10-real-endpoint-validation` | `92b46b0` | Yes | Yes | `0/380` | Already merged; archive after audit window. |
| `codex/p4-00-execution-acceleration` | `e029c2e` | Yes | Yes | `0/375` | Already merged; archive after audit window. |
| `codex/p4-01-catalog-singleflight` | `c78d182` | Yes | Yes | `0/348` | Already merged; archive after audit window. |
| `codex/p5-anthropic` | `0b5e143` | Yes | Yes | `0/335` | Already merged; archive after audit window. |
| `codex/p6-grok-build` | `8b0f0a2` | Yes | Yes | `0/304` | Already merged; archive after audit window. |
| `codex/p7-kiro` | `c76f075` | Yes | Yes | `0/296` | Already merged; external Kiro validation remains deferred. |
| `codex/p8-official` | `9249711` | Yes | Yes | `0/275` | Already merged; external Official E2E remains deferred. |
| `codex/plan-execution-acceleration` | `5e7c6e7` | Yes | Yes | `0/379` | Already merged; archive after audit window. |
| `main` | `cdfff7a` | Yes | Yes | `0/0` | Canonical default branch; retain. |

## Local-only branch divergence

The local `codex/p13-10-websocket` tip was `c9d490e`, seven commits ahead of its remote
`3dc5dc5`. Those seven commits are already ancestors of the integration candidate. Do not force
push the old phase branch merely to synchronize it; merge the integration candidate and retain the
immutable phase tags.

## P13-08 reconciliation record

The three P13-08-only commits are:

- `04a8d31` — one-line billing catalog read-context correction; later active history contains the
  patch-equivalent correction (`bf6c9fd`/subsequent management code).
- `7e14a27` — historical phase tag preparation and preflight/review receipts.
- `b9d0a89` — historical plan/ADR/contract/traceability closeout updates.

The active plan, reports and traceability already record P13-08 as formally gated. Preserve the
immutable P13-08 tag/evidence and avoid a whole-branch merge that could reintroduce stale versions
of files later changed by P13-09 through P13-11.
