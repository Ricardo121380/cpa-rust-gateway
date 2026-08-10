# CI delivery-gate split

- Change boundary: `CR-EXEC-008`
- Date: 2026-08-10

## Policy implemented

The normal workflow now provides a lightweight PR/main safety net (documentation/source policy,
format and workspace check). A separate delivery workflow runs the expensive Fast, Full and
supply-chain checks only for a phase closeout tag, an explicitly dispatched run, or a PR carrying the
`delivery-closeout` label. The label authorization is bound to the exact PR head SHA. A subsequent
PR synchronize with a stale label fails the required delivery job and requires re-authorization.

This reduces expensive runner executions; it does not reduce the frequency of branch pushes, PR
creation/update, review, rebase or merge. Those Git operations remain available whenever collaboration
requires them.

## Local evidence

The following checks passed on the same worktree revision:

- `ruby -c scripts/check-ci-workflow.rb`
- `ruby scripts/check-ci-workflow.rb`
- `ruby scripts/check-release-artifact-workflow.rb`
- workflow shell/classifier/source/crate/secret checks
- `cargo fmt --all -- --check`
- `cargo check --locked --workspace --all-targets --all-features`
- `./scripts/check.sh fast` (including workspace Clippy/tests, management SPA, P12 envelope and
  secret/document checks)

No expensive Delivery Gate was manually started for this closeout. After the branch push, the normal
PR workflow auto-triggered as designed: scope classification passed, the lightweight compile job was
queued, and Docs-only plus all Fast/Full/supply-chain Delivery jobs were skipped. The repository
branch-protection API returned HTTP 403 under the current private-repository plan, so the old required
context cannot be claimed as migrated. The repository owner must review required contexts when that
API is available.

Verdict: `PASS_LOCAL_WITH_REQUIRED-CONTEXT-BOUNDARY`.
