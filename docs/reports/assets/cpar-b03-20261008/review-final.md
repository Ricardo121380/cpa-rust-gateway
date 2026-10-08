# B-03 final independent two-axis review

Fixed base: `9265e6cea83b6172742dc2e4e60b5e5d94c1a1c2`.
Final source: `5e51f5f170a01b813631824b004eb2fcbf190201`.
Full diff: `git diff 9265e6cea83b6172742dc2e4e60b5e5d94c1a1c2...5e51f5f170a01b813631824b004eb2fcbf190201`.
Commit list: `edb4a93f8689ea3986917ec8befbeec297260889` and
`5e51f5f170a01b813631824b004eb2fcbf190201`, both titled `B-03: ...`.

Reviewers were separate new generic, read-only, one-shot agents with explicit
`fork_turns=none`, no model/role override. Main waited, froze source, and owned all
modifications, decisions and executed tests. Each final review combines the prior
complete full-diff review with independently checked equality of every final blob.
This is composed coverage, not a claim that every unchanged file was reread twice.
The prior FAIL and the intervening PARTIAL remain in their original reports.

## Standards

Agent: `/root/b03_standards_title_gap`; terminal `complete`.

独立 **Standards PASS；标题 finding CLOSED；硬 finding 0；非阻断建议 1；coverage COMPLETE 15/15**。范围仅 B-03 固定 base `9265e6cea83b6172742dc2e4e60b5e5d94c1a1c2` → source/HEAD `5e51f5f170a01b813631824b004eb2fcbf190201`。

以下 `A/` 指 `docs/reports/assets/cpar-b03-20261008/`。

- **标题事实**：已读适用根 `AGENTS.md`、`docs/06-development-plan.md:1448`，原文“Commit 标题以 Task ID 开头”。直接读取四个 commit 对象，新标题分别为 `B-03: reject Responses terminal output order conflicts`、`B-03: keep Responses bootstrap protocol failures nonretryable`；两对 author、committer、日期均相同。首条正文确实省略，精确旧正文与备份 commit 完整保留（`A/commit-title-correction.json:8`、`:82`、`:86`）。这属于已披露消息变更；该标题规则没有要求保留正文，标题硬 finding 已闭合，不能描述为“only title changed”。
- **保全事实**：实际查询具体 `refs/codex/b03/pre-title-correction/first`、`.../final`，分别指向 `4dd28268e277fa5b223c090ebca73a4707b32505`、`4b6959b3adb536f2219051799324679add16aebf`。first 新旧 tree 均 `2ade02acfd064c7308ef4f39ea97fae98a6a7c61`；final 新旧 tree 均 `33726d90fc517fae131819f601013dbd727fe776`。逐一执行全部 15 路径的 `git rev-parse SHA:path`，**15/15 blob 相等且与 correction 记录一致**（`A/commit-title-correction.json:20`–`:80`）；旧、新 fixed-base 路径集合相同。全文重读原 Standards（`A/review-before-title-correction.md:9`–`:28`），通过上述独立 parity 继承其 15 路径及 12 smell 覆盖。
- **保留建议**：实际点验 `apps/gateway/src/runtime/batch_c/http/responses_done/responses_order.rs:125`、父 `responses_done.rs:771`：事件快照、SQLite append/reopen、Request/Attempt 失败断言仍是局部测试辅助重复。possible DuplicatedCode 仍属判断性建议，无新增硬违规依据。
- **回执事实**：`A/full-receipt.json:2`、`:9`、`:28`、`:34`–`:48` 记录新 source 执行、前后 clean、源码 hash unchanged、EXIT0、45/45、1477/0/12；实际核验 **15/15 source SHA256**、报告 45 PASS 行与日志计数。原始 `output` 字段 UTF-8 SHA256=`994dcdc6266aaf4b82f70ee15c614fc4c58038177938c7cda3ea257ac330cc66`，报告 SHA256=`13f252488c958c3a25b46d31f4d3765b7d95067db4379e0463323d381dee5497`，均匹配。`A/targeted-binding.json:5`–`:95` 的四份 receipt 文件及内部 output hash 全匹配；相关 product hashes 全匹配新 source，实际旧执行 HEAD 保留，未冒标新 HEAD。

实际重读范围为上述规则、原 review、元数据/回执、两处建议代码；无遗漏缺口。未重新全面阅读不变源码，未运行测试、修改文件；此次 PASS 不扩展为发布或生产验收。

## Spec

Agent: `/root/b03_title_closed_spec`; terminal `complete`.

独立 **Spec verdict：PASS；coverage COMPLETE，15/15**。固定范围：`9265e6cea83b6172742dc2e4e60b5e5d94c1a1c2...5e51f5f170a01b813631824b004eb2fcbf190201`。(a)未发现规格缺失／部分实现；(b)未发现范围扩大；(c)未发现实现错误。

覆盖继承有实证：实际读取 `docs/reports/assets/cpar-b03-20261008/review-before-title-correction.md:32–49` 的独立终态、原审查源码及全部15路径；逐路径查询 Git 对象，确认旧 `4b6959b3adb536f2219051799324679add16aebf` 与新 HEAD **15/15 blob 相等**，且全部匹配 `commit-title-correction.json:16–76`。最终 tree 均为 `33726d90fc517fae131819f601013dbd727fe776`；首提交 tree 也相等，两对 author／committer／日期全部不变。保留 refs 实际为 `refs/codex/b03/pre-title-correction/{first,final}`。

本轮重读契约、Task card、报告，并点验 `upstream_response.rs`、`runtime.rs`、`responses_done.rs`、`responses_order.rs` 关键分支；`batch_c.rs` 与七个证据附件不重复通读，继承上述完整覆盖及已独立核实的 blob equality。未覆盖路径：无。

关键依据：原规格“verify terminal sequence rather than set equality alone”（`docs/reports/assets/cpar-release-readiness-20261005/review-results.md:149`）；`crates/protocol-openai-responses/src/upstream_response.rs:784,1471` 校验既定位置及精确 metadata，`:568` 保持永久拒绝。契约 `docs/contracts/BC-PROTOCOL-001-openai-responses-adapter.md:214–227` 保留交错、兼容省略及运输 EOF；`apps/gateway/src/runtime.rs:7312` 保留 UPE NonRetryable。公开失败／存储／WS／实际 receive/handoff 断言与前轮一致。

Full 已核对原始输出、报告 SHA、15文件 SHA、当前隔离 checkout HEAD／空 status：`full-receipt.json:2,9–28,34–48` 对应新源码真实新运行，**EXIT0、45/45、1477/0/12**；与旧运行时间及输出 hash 不同。定向回执保留其实际旧 HEAD，按真实文件 SHA 继承 **26/0/0、strict Clippy EXIT0**，没有改标执行来源。本探子未运行测试。

元数据事实：`git cat-file commit` 显示首提交 `4dd28268…→edb4a93f…` 同时删除了原提交正文；“只改标题”描述不精确，源码未变，不构成 Spec finding。已提交报告仍 IN_PROGRESS；B04、整体发布及真实 Provider／生产不在本 PASS 内。

## Main disposition and coverage

The exact 15-path scope is the list in `review-coverage.json`. The documentation
closure that saves these reports and the completed delivery report follows the
source reviews; it is subject to the local documentation gate, not retrospectively
claimed as new source-review coverage.

Main accepted the one nonblocking testing-helper duplication advisory without
extra factoring: both explicit failure oracles stay readable, and this avoids an
unnecessary refactor after accepted source validation. This is an open advisory,
not an open hard finding or a hidden fix. The original hard title finding is closed.
Original bodies/messages, failure/PARTIAL outcomes and all intermediate Full runs
are preserved; none is promoted to a different verdict.

Standards: 0 open hard findings, 1 nonblocking DuplicatedCode advisory; Spec: 0 findings.
