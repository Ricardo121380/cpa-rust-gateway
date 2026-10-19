# Branch names and retained history

The current development line is `develop`. GitHub's default branch remains `main`.
As of the 2026-10-19 cleanup, `main` contains a separate sanitized two-commit
history; it does not contain the current development line. Do not infer current
feature availability from the default-branch page.

## Retained branches

Local and GitHub branch names use the same scheme:

| Branch | Purpose |
|---|---|
| `main` | Existing public sanitized history; remains the GitHub default |
| `develop` | Current integration and ongoing development |
| `release/execution-arm64-20261002` | Historical signed candidate at `80f5838902eb56be6dccc3ce047eb227d6c18e6c` |
| `research/channel-workflows-20260929` | Independent channel workflow research and its retained worktree |
| `research/channel-evidence-20260929` | Independent channel evidence research and its retained worktree |
| `prototype/account-flows-20260929` | Independent account flow prototype and its retained worktree |
| `archive/grok-pi-reasoning-20260923` | Historical hotfix with one commit outside the development baseline |
| `archive/schema27-rollback-20261019` | Historical rollback line with four commits outside that baseline |
| `archive/channel-pin-gate-20261019` | P13-08 closeout history with three commits outside that baseline |

The outside-history counts refer to development baseline
`8352b01a13a7673cc5200b86fb00a3cf81bbc2bc`. The research/prototype branches also
contain independent commits and must not be discarded merely because their
worktrees are clean. Historical release and rollback references do not establish
current production state or permission to deploy or roll back.

## Naming and lifecycle

- Use `feature/<topic>` or `fix/<topic>` for new implementation work based on
  `develop`. Use lowercase words separated by hyphens.
- Use `research/<topic>-<YYYYMMDD>` and `prototype/<topic>-<YYYYMMDD>` for work
  with an independent question or experiment.
- Use `release/<topic>-<YYYYMMDD>` for a specifically identified release candidate.
  Preserve its source SHA and artifact evidence; the name alone is not acceptance.
- Use `archive/<topic>-<YYYYMMDD>` to retain history that is still needed but is
  no longer an active development line. Do not continue routine development there.
- The date identifies the original work or the archival snapshot. The two archive
  names ending in `20261019` identify this cleanup's retention date.
- Avoid tool/author prefixes such as `codex/` and `claude/`, and avoid repeating
  the repository name in branch names.
- Delete completed temporary branches after proving their commits are reachable
  from a retained remote branch and checking open PRs, worktrees and operational
  references. Preserve an exact name-to-SHA recovery record first.
- Synchronize completed, reviewed commits to the corresponding named remote
  branch. Uncommitted work remains local until it is reviewed and explicitly
  included; never stage an entire dirty checkout just to make it look synchronized.

Development and public `main` have unrelated roots. Their eventual integration
requires a separate decision. Do not force-push `main`, merge unrelated histories,
or use `git push --all` / `git push --mirror` for routine maintenance.

## 2026-10-19 cleanup record

The retained set has nine branches. The cleanup starts from 67 local names and
22 GitHub heads; it removes 13 redundant remote heads and renames eight retained
remote heads. Locally, seven retained branches are renamed, the historical
release candidate gains a matching local branch, and local `main` is recreated
at the existing GitHub `main` tip. The previous local `main` commit remains
reachable from `develop`.

The [exact branch mapping](reports/git-branch-cleanup-20261019.json) records all
original local/remote names and full SHAs, the rename map and the removed names.
Its retained SHAs are the values before the documentation commit; `develop`
continues to advance. All original branch commits are reachable from the retained
set. A verified full Git bundle and a separate backup of the existing uncommitted
files are also retained in the private local maintenance directory. Those backups
are on the same disk and do not replace an independent backup.

GitHub renames preserve ordinary browser redirects. Other clones must update
their local branch and upstream names; Git pull and raw-file URLs do not follow
those redirects. For an existing development checkout, first verify its branch
and dirty state, then use:

```sh
git branch -m codex/prism-v4-delivery develop
git fetch origin --prune
git branch --set-upstream-to=origin/develop develop
```

These are migration instructions for other clones; the maintained local checkout
already uses the new names. See GitHub's [branch rename guidance](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-branches-in-your-repository/renaming-a-branch).

Historical reports, workflow runs and signing identities retain their original
branch strings. In particular, release signing identities include an exact
`refs/heads/...` value; do not rewrite an old receipt after a branch rename.
The existing CI push trigger is still limited to `main`, so synchronization of
`develop` is not evidence that GitHub CI ran for that revision.

The [August 19 inventory](git-branch-inventory-2026-08-19.md) remains a historical
snapshot. Use this policy and live refs for current branch names.
