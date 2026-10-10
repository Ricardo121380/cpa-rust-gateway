# Channel Pin snapshot repair review

Reviewed range: `a3dc0c235f87b50685da72d0009d131d8e1118b5...f346a03f912b150006a288c38087550bb3d04cb0` (three files, 32 additions, four deletions).

Two independent one-shot read-only reviews completed. They did not execute tests, contact Providers, or modify files. These verdicts cover this diff, not project-wide release, deployment, or production readiness.

## Standards: PASS

Hard findings: 0. Smell findings: 0.

The serving scheduler provides an immutable `Arc<RouteSnapshot>`; Pin reads it once and retains it for route lookup, model resolution, and access checks (`apps/gateway/src/runtime.rs:1401-1427`). This matches ADR-0088's serving-composition requirement and ADR-0007's single-snapshot publication rule. The fixture's plain-text branch is limited to requests without tools or reasoning (`scripts/acceptance/cpar_seo_ui_http.py:102-103`). Process validation counts independent receiver files and checks rejected Pins add zero receives and successful JSON/SSE Pins add one each. All changed paths are within backend ownership and do not alter shared management API types.

## Spec: PASS

Hard findings: 0.

The repair changes the snapshot source without removing model, client-key, group, or credential checks (`runtime.rs:1401-1430`). Config/generation checks and exact lease identity remain. The scheduler still revalidates provider, channel, credential, health, quota, expiry, and capacity. Pin still has no egress refresh and uses `start_pinned_once`; retry remains disabled and maximum attempt count remains one. The narrow Grok Build credential-continuity rule is unchanged. Fixture JSON/SSE encoding and existing tool/reasoning behavior remain shared.

## Evidence reviewed

- `pin-fixed-source-background.json` and `pin-fixed-source-background-run.json`: fixed source, clean before/after, exit 0; default and disabled modes each have JSON/SSE Pin receive deltas of one and three rejection deltas of zero. Disabled mode observes 65 seconds with zero automatic refresh/catalog calls.
- `full-f346a03f-run.json` and associated logs: fixed-source Full 45/45, Rust 1501 passed, zero failed, 12 ignored.

These are local/synthetic evidence. Real Grok Build inference had not yet been executed on the repaired runtime at review completion.
