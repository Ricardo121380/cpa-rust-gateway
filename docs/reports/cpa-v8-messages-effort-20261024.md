# CPA v8 Messages effort 实施与验证

日期：2026-10-24（Asia/Shanghai）。本页记录已授权的 effort 提取/优先级切片；
当前队列仍以[统一入口](../engineering-roadmap.md)为准。

## 源码与范围

- CPAR：`worktrees/cpar62-client-metadata-20261020`，`feature/62-client-metadata`，
  HEAD `ab78c65ca68978bf46c7cf530ea795417d8431f3` **加继承的本地修改及本次增量**。
  HEAD 本身不是本次受测源码；此前后端整理、前端退役、ADR-0125 和证据文件均保留。
- CPA：v8.0.40，`d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683`，
  [extractClaudeConfig](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/thinking/apply.go#L740)。
  使用已固定的源码归档；本次没有重新查询浮动 main 或运行 Go 双臂差分。
- 生产改动仅在 [compatibility::thinking](../../crates/gateway-router/src/protocol_transform/compatibility.rs)。
  测试沿用 [Router 回归](../../crates/gateway-router/tests/cpa_request_compatibility.rs)与
  [公共 HTTP receiver](../../apps/gateway/src/runtime/batch_c/http/cpa_compatibility.rs)。
  行为决定见 [ADR-0126](../adr/ADR-0126-cpa-v8-messages-effort.md)，契约见
  [BC-PROTOCOL-015](../contracts/BC-PROTOCOL-015-local-messages-request-controls.md)。

本次完整门禁受测 manifest：`candidate-final-manifest.json`，SHA-256
`92dd15bd1c5e90e6bc57cf6a54a1996a3c3332be47bc06bcfe2a918a2fafcd61`。
验证结束后的文档结果补记与受测 Rust/lock 身份分开，最终校验确认源码未再变化。

本地回执目录为工作区根下 `maintenance/2026-10-23-messages-effort-v8/`；目录沿用开始准备时的命名。
`baseline.json`、`before/`、`index-before.bin` 保存本次修改前状态；`tested-files.json`
绑定生产文件、三个测试文件和 Cargo.lock。candidate-manifest 保存继承 overlay 的完整路径/内容/模式身份；最终 preservation 回执核对其他所有原有路径及 index。
这些回执属于本地工作区，不假定远端仓库能访问。

## 行为变化

| Messages 输入 | 之前 | 本次结果（Chat / Responses） |
|---|---|---|
| 只有 output_config.effort=" HIGH " | 忽略 | high / high |
| enabled、无预算、effort=high | auto / medium | high / high |
| enabled、无预算、无有效 effort | auto / medium | auto / auto |
| enabled、预算1024、effort=high | low / low | 保持 low / low，预算优先 |
| disabled、effort=high | none / none | 保持 none / none，禁用优先 |
| 无 thinking、无或空白 effort | 不生成 thinking | 保持不生成 |
| native Messages 原生透传 | 保留源 JSON 值 | 保持，包括 effort 的空白和没有 thinking |

明确的 none/auto 不丢失；adaptive/auto 无有效 effort 仍使用已有 xhigh 翻译默认值。
没有新增 summary 意图、配置开关、依赖、迁移或模型资格。CPA 的 source extractor、
Chat/Codex translator 和 model applier 各有职责，默认值还受 resolved model、用户模型和
payload override 影响。本次不宣称完整执行器等价或模型专用 clamp。
CPAR 仍拒绝非字符串/未知 effort、未知 output_config 成员和0/-1预算；这些差异明确保留。

## 验证

先新增回归再修改生产代码。RED 中 effort-only 的实际结果为 None，期望为 high；
第二个负向测试也暴露了 effort-only 的未知等级被忽略。原始失败保存在 `red-router.log`，退出码101。

首次全量回归另发现 Build 旧测试仍要求 effort-only 没有 reasoning，触发失败。该旧期望
已按 ADR-0126 改为完整 `reasoning={"effort":"high"}`，原有元数据留存、Usage/账本及
租约释放断言保留；测试名称与 BC-PROTOCOL-016 同步。首次失败单独保留在
`full-checks.log`。重命名测试触发的 rustfmt 换行要求已修复，检查记录保留于
`format-check.log`。最终候选另跑 `full-final.log`，不覆盖旧失败。

| 检查 | 结果 / 覆盖 |
|---|---|
| Router compatibility | PASS，9/9；新增提取/优先级表含96组合，另有已知控制、能力、LosslessBridge与预算边界负例 |
| 公共 HTTP effort / native | PASS，2/2测试函数；40跨协议正向请求、10原生正向请求、12负例 |
| 公共响应 | 跨协议请求检查完整 JSON/SSE 文本和成功 terminal；独立 loopback socket receiver 检查实际目标字段 |
| 零新增发送 | 非字符串 effort、未知等级、孤立 tool_result 返回400；handoff、receiver、Attempt 均没有增加 |
| Build effort-only 与 M3/M4 | PASS；JSON/SSE 检查实际 reasoning、私有元数据、Usage/账本及释放 |
| 快速代码门禁 | PASS，完整脚本中的 fast 部分41/41，包括格式、Clippy、Rust测试、进程/多轮、源码/秘密/边界及文档契约检查 |
| Workspace Rust tests | PASS，132个测试集汇总1671 passed、0 failed、21 ignored；ignored 不计为通过 |
| Full 供应链门禁 | FAIL；Quality tool versions通过，cargo deny因 Tokio 公告失败；完整脚本记录42 PASS、1 FAIL，并在失败处停止 |
| 独立 cargo audit --json | 退出码0，vulnerabilities=0，但有1项 unsound warning；不能据退出码否定 cargo deny 的阻断 |

定向日志：`green-router.log`、`http-effort.log`。上游发送经正式 builder/client pool，测试仅将
已准入目的地改向独立本地 HTTP peer；公网 DNS、真实 TLS、Provider 推理和实际客户端不在此证据内。

## 当前供应链阻碍

继承的 Cargo.lock 固定 Tokio **1.53.0**，本次依赖文件未变。`cargo deny check` 命中
[RUSTSEC-2026-0386 的固定公告](https://raw.githubusercontent.com/RustSec/advisory-db/84d943a6deeb8cc6c6f632531aaf4f1c62275239/crates/tokio/RUSTSEC-2026-0386.md)：
AsyncFd 构造器的 I/O safety 问题；公告修复范围包括 **>=1.53.3**。
审计库 commit 为 `84d943a6deeb8cc6c6f632531aaf4f1c62275239`。

完整门禁保持 **FAIL**。独立 audit 将它归为 informational unsound 并以0退出，项目 deny
规则仍将该项阻断；没有添加 ignore、降低门槛或将忽略当修复。本次只实现 effort 切片，
依赖升级作为独立安全维护登记，发布前需修复并重跑供应链/受影响回归。此处没有做可利用性结论。
回执为 `full-final.md`、`full-final.log`、`audit.json` 和 `audit.stderr`。

## 剩余资格

实际 Codex/Pi/Claude、固定 Build 服务、真实 Provider、目标 binary/OCI、远端 CI、发布部署
均 **NOT_RUN**。原有首版、#61–#67 条件及 cost_in_usd_ticks NOT_ADMITTED 未改变。
本次没有 GitHub 写入、提交、推送或部署；最新候选需要的客户端验证仍留在当前队列。
