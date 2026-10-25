# Documentation map

Use this page to locate project documents without moving historical evidence or
changing its original paths and revision references.

## Start here

| Question | Entry point |
|---|---|
| What does CPAR provide? | [English README](../README.md), [中文 README](../README.zh-CN.md) |
| What exists, what is wired into production, and what comes next? | [Current status and development order](engineering-roadmap.md) |
| What requirements and phase history exist? | [Development plan](06-development-plan.md), [traceability](traceability.md) |
| Where do files and local work belong? | [Repository and workspace layout](repository-layout.md) |
| Which branch contains current development? | [Branch names and retained history](branch-policy.md) |
| How is the system structured? | [Current backend architecture and CPA mapping](03-target-architecture-draft.md), [crate boundaries](crate-boundaries.md) |
| Which behavior is required? | [Behavior contracts](02-behavior-contracts.md), [contract index](contracts/README.md) |
| Why was a design chosen? | [ADR index](adr/README.md) |
| How is a change verified? | [Quality gates](quality-gates.md), [verification reports](reports/README.md) |
| How is CPAR operated? | [English deployment guide](deployment-guide.en.md), [中文部署指南](deployment-guide.zh-CN.md), [P12 rollout runbook](p12-rollout-runbook.md) |

## Planning, integration and history

The [current roadmap](engineering-roadmap.md) is the single summary of implementation,
production wiring, qualification, the pinned CPA comparison version and next work.
The architecture map locates code; [the candidate inventory](support/first-release-candidates.md)
preserves client and delivery evidence with its source identity.

- [Historical development plan](06-development-plan.md) and
  [traceability](traceability.md) preserve phase requirements and their evidence.
- [The October 18 roadmap](reports/engineering-roadmap-20261018.md) preserves the former
  planning snapshot. [The CPA v8 comparison](reports/cpa-v8-functional-comparison-20261023.md)
  and [the statistics ecosystem comparison](reports/cpa-ecosystem-comparison-20261023.md)
  are dated research, not additional active task queues.
- [Management frontend design](07-management-frontend-design.md) and
  [frontend development plan](08-management-frontend-development-plan.md) are historical Prism material.
- [Cross-boundary log](cross-boundary-log.md) records frontend/backend handoffs.
- [Reference baseline](00-reference-baseline.md),
  [historical feature selection](01-feature-selection-matrix.md),
  [channel reference analysis](04-channel-reference-analysis.md) and
  [upstream aggregation design](05-upstream-aggregation-design.md) retain source decisions.
- [Current branch policy](branch-policy.md) identifies `develop`, the separate
  public `main`, retained research and historical references.
- [The August 19 branch inventory](git-branch-inventory-2026-08-19.md) is a dated
  audit. Its branch names and merge recommendations are historical.

Dates and commit IDs in reports define their evidence scope. A historical PASS
does not establish that the current checkout, GitHub branch, release or production
runtime passed the same checks.
