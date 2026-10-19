# Repository and local workspace layout

This guide separates versioned project deliverables from local working material.
It does not change package boundaries, release policy or the authority of existing
contracts and acceptance reports.

## Inside the source repository

| Location | Contents | Git treatment |
|---|---|---|
| `apps/`, `crates/`, `web/prism/` | Product source and tests | Versioned; preserve the ownership rules in `AGENTS.md` |
| `deploy/`, `scripts/`, `tools/` | Reviewed deployment assets, checks and tooling | Versioned; credentials remain external |
| `docs/adr/`, `docs/contracts/`, `docs/openapi/` | Decisions and authoritative contracts | Versioned |
| `docs/reports/` | Reviewed reports, redacted receipts and selected evidence assets | Versioned after content review |
| `docs/design/`, `docs/handoffs/`, `docs/change-requests/` | Design material and coordination records | Versioned when ready; retain existing work in progress |
| `output/` | Local evidence collection, historical checkouts, logs and scratch runs | Ignored; may contain irreplaceable evidence and nested Git repositories |
| `.playwright-cli/`, `test-results/` | Local browser and test captures | New files ignored; existing tracked captures remain versioned |
| `.codegraph/` | Local code index | Ignored |
| `target/`, `node_modules/`, `dist/` | Build and dependency output | Ignored; check evidence references and active use before cleanup |

An ignored file is still local data. Ignoring it neither archives it nor makes it
safe to delete. In particular, never treat all of `output/` as disposable cache.
Publish a reviewed, redacted report and its necessary receipts under `docs/reports/`
instead of force-adding an entire local output tree. Keep raw Provider data,
credentials, production databases and private recovery material outside Git.

Existing untracked documentation and scripts remain visible for ownership review.
Do not use blanket ignore rules for `docs/`, `docs/reports/` or `scripts/` to make
the working tree look clean.

## Beside the source repository

The local project container can hold several independent repositories. Its
filesystem root is not necessarily the product Git root. Check
`git rev-parse --show-toplevel` before running repository commands.

```text
CPA-Rust/
  README.md                         local navigation and maintenance record
  repo/cpa-rust-gateway/             active product repository
  repo/prism/                        separate reference repository
  worktrees/                        research and prototype checkouts
    validation/                     retained verification worktrees
  maintenance/                      local inventories, Git bundles and recovery records
  output/                           compatibility entries for historical validation paths
```

Historical paths may remain as compatibility symlinks after a move. Do not rewrite
old receipt `cwd` values or commit identities: they describe the original run.
Record the new path separately and verify that old references still resolve.

Move a registered checkout with `git worktree move`, then verify its HEAD, status,
Git registration and preserved content. Ordinary `mv` does not update Git's
worktree bookkeeping. Do not move a checkout used by an active process. Nested
repositories and worktrees inside an evidence tree need their own inventory.

For new work, keep verification checkouts under `worktrees/validation/` and avoid
creating more source copies inside the product repository. Give every evidence run
a descriptive task/date directory. Use the existing build output location unless
the verification procedure requires a distinct build directory.

## Git synchronization and recovery

The active development branch is `develop`; local and GitHub names follow the
[branch policy](branch-policy.md). Public `main` remains a separate history.

Before changing refs or moving material, record HEAD, branch, index, dirty paths,
worktree registrations and the live remote refs. A local Git bundle preserves
referenced commits; it does not include uncommitted or ignored files. Save those
separately according to their role and sensitivity. A backup on the same disk does
not provide protection against disk loss.

Fetch remote refs before assessing divergence. Stale remote-tracking refs can be
pruned after their identities and reachable history have been preserved. Keep
local branches and historical evidence until their disposition is explicit.

Push a fixed, reviewed commit to a named branch only within the authorized scope.
Inspect the relevant workflow triggers, scan every newly published blob version,
check file sizes and read back the remote SHA afterwards. Keep GitHub synchronization
separate from feature acceptance, release, deployment and real Provider validation.

As observed on 2026-10-19, public GitHub `main` has a separate sanitized two-commit
history, while the local development history has a different root. Do not force
push local `main`, merge unrelated histories, or use `push --all`/`--mirror` as a
cleanup operation. The integration method needs an explicit decision after both
histories have been reviewed. Ordinary development-branch synchronization can be
handled independently.

After the branch cleanup, local `main` tracks that existing public history. Its
previous tip remains in `develop`, with the exact old ref recorded in the cleanup
mapping. Changing the local ref did not merge or rewrite GitHub `main`.

The 2026-10-19 local maintenance directory contains the exact before/after inventory,
backup scope, worktree relocation map and verification receipts. Those private local
records are not part of the public repository.
