# Issue tracker: GitHub

本仓库的 Issues 和 specs 使用 GitHub Issues：
`Ricardo121380/cpa-rust-gateway`。使用 `gh` CLI 操作。

以下命令从本仓库根目录执行；从其他目录执行时，显式添加
`--repo Ricardo121380/cpa-rust-gateway`。

## Conventions

- 创建：`gh issue create --title "..." --body-file <body.md>`
- 读取正文、标签及评论：
  `gh issue view <number> --json number,title,body,labels,comments,state,assignees`
- 列出：`gh issue list --state open --json number,title,body,labels,comments`
  按任务添加 `--label` 或调整 `--state`。
- 评论：`gh issue comment <number> --body-file <comment.md>`
- 添加标签：`gh issue edit <number> --add-label "..."`
- 移除标签：`gh issue edit <number> --remove-label "..."`
- 关闭：`gh issue close <number>`

多行正文先写入文件，再使用 `--body-file`，保留换行并避免 shell 插值。
分诊标签映射见 `triage-labels.md`。

这些约定定义操作方法；写入、评论、关闭、分配工单等远程变更，
仍须由当前任务的明确授权覆盖。

## Pull requests as a triage surface

**PRs as a request surface: no.**

GitHub 的 Issues 和 PR 共用编号空间。引用只有编号时，先确认对象类型。

## When a skill says "publish to the issue tracker"

创建 GitHub issue。

## When a skill says "fetch the relevant ticket"

读取对应 issue 的正文、标签和评论。

## Wayfinding operations

供 `/wayfinder` 使用：一个 map issue，加若干 child issues。

- Map：使用 `wayfinder:map` 标签；正文维护 Notes、
  Decisions-so-far 和 Fog。
- Child：优先使用 GitHub sub-issues 关联 map；不可用时，在 map
  中维护任务列表，并在 child 正文顶部记录 `Part of #<map>`。
  类型标签使用 `wayfinder:research`、`wayfinder:prototype`、
  `wayfinder:grilling` 或 `wayfinder:task`。
- Blocking：优先使用 GitHub 原生 issue dependencies；不可用时，
  在 child 正文顶部记录 `Blocked by: #<n>, #<n>`。
  所有阻塞工单关闭后才视为解除阻塞。
- Frontier：按 map 顺序选择首个 open、无未关闭 blocker、
  且未分配的 child。
- Claim：在开始工作前，用 `gh issue edit <number> --add-assignee @me`
  认领。
- Resolve：评论记录结果，关闭 child，再向 map 的
  Decisions-so-far 添加结论摘要和链接。

需要 API 操作时使用 `gh api`，核对当前接口及权限；
区分 issue 编号、数据库 ID 和 node ID。
