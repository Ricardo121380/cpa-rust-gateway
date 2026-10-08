# Independent reviews before commit-title correction

Fixed base: `9265e6cea83b6172742dc2e4e60b5e5d94c1a1c2`.
Reviewed source: `4b6959b3adb536f2219051799324679add16aebf`.
Both reviewers were new generic, one-shot, read-only probes with `fork_turns=none`.
Main waited while they ran; neither edited files or executed tests. The following
terminal reports retain the separate findings and original verdicts.

## Standards

Agent: `/root/b03_final_standards`; terminal `complete`.

Standards verdict：**FAIL**。完整覆盖 **15/15**；源码及文档 hunk 未发现硬违规，发现一项提交元数据规范违反。固定比较：`9265e6cea83b6172742dc2e4e60b5e5d94c1a1c2...4b6959b3adb536f2219051799324679add16aebf`，HEAD/分支均与任务指定一致。

- **硬违规，P3，提交标题**：`docs/06-development-plan.md:1448` 原文：“Commit 标题以 Task ID 开头，例如 `P1-03: add canonical event state machine`。”实际 `4dd28268` 标题为 `fix(cpar): reject B-03 Responses terminal output order conflicts`；`4b6959b3` 标题为 `fix(cpar): keep B-03 Responses bootstrap protocol failures nonretryable`，均未以 `B-03` 开头。所检查的规范源未找到此规则的明确例外。
- **判断性建议，possible Duplicated Code**：`apps/gateway/src/runtime/batch_c/http/responses_done/responses_order.rs:125` 与 `apps/gateway/src/runtime/batch_c/http/responses_done.rs:771` 重复事件快照→SQLite append→重新打开读取→Request/Attempt 失败断言。关键原文：`SqliteEventStore::open(&database)?.append_batch(&events)?;`、`SqliteEventStore::open(&database)?.list_events()?;`。这是测试辅助逻辑的局部重复，不构成硬违规。

实际覆盖路径（每组共同前缀加所列文件名）：

- `apps/gateway/src/`：`runtime.rs`、`runtime/batch_c.rs`、`runtime/batch_c/http/responses_done.rs`、`runtime/batch_c/http/responses_done/responses_order.rs`。
- `crates/protocol-openai-responses/src/upstream_response.rs`。
- `docs/contracts/BC-PROTOCOL-001-openai-responses-adapter.md`。
- `docs/reports/assets/cpar-b03-20261008/`：`bootstrap-fallback-red-binding.json`、`bootstrap-fallback-red-harness.patch`、`bootstrap-fallback-red.json`、`official-source.json`、`red-binding.json`、`red-harness.patch`、`red.json`、`task-card.md`。
- `docs/reports/cpar-b03-responses-output-order-20261008.md`。

未覆盖项：无。规则检查范围为适用 AGENTS、README Contributing、开发计划工程流程、quality-gates、crate-boundaries、cross-boundary-log 尾部及 Task card；12 项 smell baseline 均已对照。用户 AGENTS 改动及无关未跟踪文件未计入 diff。

主代理回执只读核对：最终源码绑定、batch_c 26/0/0、Clippy EXIT0、干净 checkout Full 45/45、1477 passed/0 failed/12 ignored；没有复跑测试。磁盘 GREEN/Full 回执未冒充上述已提交文件覆盖。

## Spec

Agent: `/root/b03_final_spec`; terminal `complete`.

独立 Spec verdict：**PASS**；审查覆盖 **COMPLETE，15/15**。固定范围为 `9265e6cea83b6172742dc2e4e60b5e5d94c1a1c2...4b6959b3adb536f2219051799324679add16aebf`，未覆盖路径：无。此范围内：(a)未发现规格缺失或部分实现；(b)未发现未要求行为或范围扩大；(c)未发现看似实现但实际错误的新增分支。

已核验事实：

- 原 B-03 要求“verify terminal sequence rather than set equality alone”（`docs/reports/assets/cpar-release-readiness-20261005/review-results.md:149`）。`crates/protocol-openai-responses/src/upstream_response.rs:784` 拒绝不符合既有 start 位置的显式 index；`:1471` 逐位置同时校验身份、既定 index、精确 item-done metadata，随后保留重复／遗漏／全部 ended 检查。冲突发生在 ResponseEnd 前；`:568` 的 Rejected 状态不能被后续合法终态救活。
- 契约明确“Cross-item deltas may interleave”（`docs/contracts/BC-PROTOCOL-001-openai-responses-adapter.md:214`），并保留省略兼容性。`apps/gateway/src/runtime/batch_c/http/responses_done/responses_order.rs:312` 覆盖双向完成顺序、缺最终 output、缺 index／细粒度确认及 snapshot 内容；父文件 `responses_done.rs:494` 比较完整 SSE、GET、续接 output 和实际上游历史。无新增全局事件排序。
- `apps/gateway/src/runtime.rs:7312` 将 bootstrap UPE 保留为 NonRetryable，运输 EOF 分类保持原样。双候选 GREEN 回执显示冲突为 **502、主1／备用0／Attempt1**；EOF 控制为 **200、1／1／2**。`responses_order.rs:113` 同时断言实际接收与 handoff 数；父文件 `responses_done.rs:123` 明确保留 handed-off AttemptSucceeded 与 RequestFailed 区别，`:261`、`:579` 检查失败无 completed、无可读取／续接存储、无可用 WS root。
- 静态核对回执及 SHA：最终 batch_c **26/0/0**，含 B01/B02/tool 回归；`full-receipt.json:2` 绑定最终源码，`:9`／`:28` 记录前后 clean、源码 hash 不变，`:37` 起记录 EXIT0、45/45、1477/0/12。15 个提交文件的回执 hash 均匹配固定 HEAD。本探子未执行测试。已提交报告仍为 IN_PROGRESS，未提前宣称完成；B04 Usage ownership、B05及整体发布不在此 PASS 内。

完整覆盖路径（花括号表示逐项展开，均相对仓库）：

- `apps/gateway/src/{runtime.rs,runtime/batch_c.rs,runtime/batch_c/http/responses_done.rs,runtime/batch_c/http/responses_done/responses_order.rs}`
- `crates/protocol-openai-responses/src/upstream_response.rs`
- `docs/contracts/BC-PROTOCOL-001-openai-responses-adapter.md`
- `docs/reports/assets/cpar-b03-20261008/{bootstrap-fallback-red-binding.json,bootstrap-fallback-red-harness.patch,bootstrap-fallback-red.json,official-source.json,red-binding.json,red-harness.patch,red.json,task-card.md}`
- `docs/reports/cpar-b03-responses-output-order-20261008.md`

Standards: one hard commit-title finding and one nonblocking advisory; Spec: zero
findings. Title correction and final-source recheck are recorded separately. These
reports are not silently relabelled PASS.
