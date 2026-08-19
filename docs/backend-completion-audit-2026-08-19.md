# Backend completion and publication audit — 2026-08-19

## Verdict

The implemented and approved CPAR backend is formally gated through **P13-11E4**, but the phrase
“the entire backend is complete” would be inaccurate. Three different completion layers must be
kept separate:

1. **Implemented/gated backend:** approved P0–P6, P9–P12 and P13-04 through P13-11E4 slices have
   passed their documented acceptance boundaries.
2. **Public-main/release integration:** the backend exists on pushed feature/integration branches,
   but the default `main` branch has not received the P13 integration PR and no public release
   binary/container is published.
3. **Explicitly deferred capability:** external credentials, real Provider/proxy/DNS canaries and
   later roadmap items remain deferred or unauthorized.

## Formally gated implementation

The authoritative plan and traceability records identify the following as complete within their
frozen boundaries:

- P0–P6 and P9–P11;
- the CPAR-owned P12 deployment/runtime boundary;
- P13 management operations, billing, Provider account pools, deterministic routing, Channel Pin,
  stored Responses, response continuity/compaction and Responses WebSocket;
- generic compatible egress profiles/pools/serving handoff;
- Provider-specific egress/session/clearance state seams and their read-only management projection.

The latest backend closeout evidence is:

- branch: `origin/codex/p13-11-egress`;
- branch tip: `1bb1fd7c812e8f2ada71559caf7e15c009e3e84f`;
- E4 immutable tag: `phase-p13-provider-egress-status-complete`;
- E4 closeout commit: `ce98faa9306d076f5af53b9eef0c818abb1cb9c8`;
- formal Gate: GitHub Actions run `32110872875`, Authorize/Fast/Full/Required successful.

At the time of this audit, there were no uncommitted files under `apps/`, `crates/` or `tests/`.
The dirty worktree consisted of the plan update, Claude-owned Prism development, public
documentation/deployment work and four preserved local helper scripts.

## Remaining backend or external boundaries

| Boundary | State | Explanation |
|---|---|---|
| P7 Kiro external authentication | Deferred | Local adapter code does not substitute for a fresh authorized external E2E. |
| P8 Official API-key external E2E | Deferred | Requires an operator-provided key and a separate bounded request authorization. |
| Grok Web external egress/WAF | Deferred | Local seams do not prove production Statsig/clearance/FlareSolverr or proxy-path success. |
| P13-11E5 real Provider/proxy/DNS canary | `DEFERRED_UNAUTHORIZED` | No real network activity is authorized by the completed local Gate. |
| Build/Console native fixed/pool production path | Boundary remains | E0–E3 prove typed state/attempt seams, not every production proxy topology. |
| P13-12 Autoreg handoff | Deferred external dependency | Registration, login, SSO/OAuth refresh, entitlement repair and replenishment are Autoreg, not CPAR. |
| P13-13 Media/Files/Batch | Deferred | Independent protocol/storage/security project. |
| P13-14 additional Providers | Deferred | Each Provider needs a capability/credential/egress contract. |
| Provider-native WebSocket and Realtime | Deferred | P13-10A is the CPAR downstream Responses WebSocket, not Provider-native transport or Realtime. |
| Public release/published image | Not yet implemented | Current signed artifacts are private and short-lived; no GHCR/GitHub Release exists. |

P12 should not be reopened because Autoreg has account-source problems. CPAR owns imported
credential binding, lease, Health/Quota/Circuit, cooldown, failure feedback and routing; Autoreg is
an independent project responsible for acquiring and repairing accounts.

## Why the backend was not visible on the default branch

The backend **was pushed**. It was not **merged into `main`**.

- `origin/main`: `cdfff7a`;
- `origin/codex/p13-11-egress`: `1bb1fd7`, 78 commits ahead and 0 behind main;
- `origin/claude/route-candidates`: `cd27ff3`, 81 commits ahead and 0 behind main;
- the active branch and its remote were synchronized before the current uncommitted edits.

These integration-critical remote tips and the total of 59 remote heads were revalidated on
2026-08-20 and remained unchanged.

The active SHA had no GitHub check runs because feature-branch pushes are not a trigger for the
ordinary CI workflow. CI runs on `main` pushes and pull requests; the expensive Delivery Gate is
explicitly triggered by phase tags, manual dispatch or a PR closeout label. There was no open
integration PR during the audit. The public repository also had no repository ruleset and no
classic protection on the default `main` branch.

This is a publication/integration gap, not lost backend work.

## Required publication sequence

1. Finish and independently review the current Prism changes.
2. Commit the public README/deployment and plan changes without the four local helper scripts.
3. Reconcile the old P13-08 non-ancestor branch by patch equivalence/evidence, not a blind merge.
4. Run focused checks and one clean integration Full Gate locally.
5. Add a reviewed `main` ruleset/branch-protection policy that requires the intended PR checks and
   blocks direct, unreviewed pushes.
6. Push the integration tip and open one PR from `claude/route-candidates` into `main`.
7. Let lightweight PR CI run.
8. Run one formal, revision-bound Delivery Gate for the final integration SHA.
9. Merge into `main` only after all required checks/review pass.
10. Preserve immutable phase tags; archive branches already covered by `main` after an audit window.
11. Create a separate public-release change request before publishing GHCR images or GitHub Release
    binaries.

See the [complete branch inventory](git-branch-inventory-2026-08-19.md) and the
[deployment guide](deployment-guide.en.md).
