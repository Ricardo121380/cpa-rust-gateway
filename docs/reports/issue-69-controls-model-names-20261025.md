# #69 CPA 请求控制与模型命名实施记录

日期：2026-10-25（Asia/Shanghai）。P0 与 P1 的本地实施和验收均完成，最终 Full44/44通过。远端工单状态未修改。

## 源码与范围

独立 worktree：`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar-69-request-controls-model-names-20261025`。
分支 `feature/69-request-controls-model-names`，共同基线 `84fbfbf778b582f746c1cafe31a2ce58962e8f77`。
该基线已集成 #68/#72/#74；历史基线门禁不是本票验证。

| 阶段 | 本地提交 |
|---|---|
| P0 请求控制先交付 | `551961c0` |
| P1 模型命名 | `5a837a04` |
| 首轮审查修复 | `3ed8ac01` |
| count/Explain 一致性、Clippy 和冻结 xAI 元数据 | `65d83fa0` |
| 最终数字预算范围及 receiver 回归 | `46c2807eacb4390ad7500bba5613414ddc07a365` |

固定 CPA v8.0.40 / `d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683`。
使用 `extractClaudeConfig`、BudgetToLevel/LevelToBudget、ValidateConfig、对应协议 translator、
`applyModelPrefixes`/normalizeModelPrefix、ParseSuffix/applyThinking 等符号对照。
40个去重 Claude/Codex/xAI 模型元数据来自该 SHA 的 models.json；来源 URL、原文件和投影 hash
见[来源回执](issue-69-controls-model-names-evidence/cpa-model-source.json)。本地冻结表不随上游动态 catalog 刷新，不表示账号权限或服务能力。

## 逐项验收

| 组 | 验收点（均 LOCAL PASS） | 实现/验证定位 |
|---|---|---|
| P0 | 固定 v8.0.40 源码符号、来源提取→模型规范化→wire | support 矩阵、CPA source receipt、compatibility/model_thinking |
| P0 | 0/-1、等级/预算范围、已知与未知模型 | cpa_request_compatibility；独立公共 receiver JSON/SSE |
| P0 | 原 effort 优先级复用 | ADR0126 原矩阵保留；output_config/预算冲突、坏类型先拒绝 |
| P0 | speed/service_tier 映射和无映射省略 | generic receiver + Grok receiver；native 字符串保留 |
| P0 | native、Lossless、历史、工具、输出限制 | 保留原有效 JSON，显式 lossless 仍严格；tool two cycles + continuation |
| P1 | 显式 prefix、精确 Alias、suffix 与 body 优先级 | 原 Alias 管理资源、exact-first 与全局 shadow guard、共享规范化 |
| P1 | Key/Group/真实模型/歧义与失败关闭 | public JSON/SSE、models/effective 列表、count、Explain |
| P1 | 管理验证、持久化、发布、回滚、旧固定 Snapshot | 管理 HTTP 与 publisher 回归；schema39 无 migration |
| P1 | count 名称和权限一致 | #68 共用 resolve；shared support admission；count-only transport/无 Usage |
| P1 | 工具和存储延续 | two cycles/followup；canonical optional intent、旧 payload bytes 回归 |
| 两组 | 文档/解释、Full、Standards+Spec、本地提交 | ADR0130、BC-PROTOCOL017、report 与 immutable gate receipt |

实际行为矩阵、配置/命名示例、与 CPA 的明确差异和来源见[支持文档](../support/cpa-request-controls-model-names.md)。
决策为[ADR-0130](../adr/ADR-0130-cpa-request-controls-model-names.md)，契约为[BC-PROTOCOL-017](../contracts/BC-PROTOCOL-017-cpa-controls-model-names.md)。

## 门禁和证据身份

最终受测源码 **`46c2807eacb4390ad7500bba5613414ddc07a365`**，tree `020ee5c9f2f814a0ab29dec950899b32f93ef381`。
Cargo.lock SHA-256 `a3950a764ae2dcae4e04a8a32ada596f33ea105dbfda8e9714b3e32d3216cd6d`，与共同基线相同。
`./scripts/check.sh full` 在 `2026-10-25T07:52:49.419937+00:00` 至 `2026-10-25T08:05:07.573817+00:00` 执行。
工作区在运行前后均干净，所有 tracked 输入 hash 前后一致；验证不是从历史报告继承。

| 本次检查 | 实际结果 |
|---|---|
| Full03 | **44/44 PASS**，含 fmt、Clippy all-targets/all-features、工作区测试、进程、多轮流式、策略、文档/契约、deny/audit |
| Rust tests | **1719 passed / 0 failed / 21 ignored**；133条 suite/doc-test result，不将忽略项计为通过 |
| P12 serve envelope | PASS；实际构建并启动隔离状态目录和动态 loopback 端口的 gateway |
| Agent multi-turn and stream | PASS；本地受控回归，不是真实客户端或真实 Provider |
| 最终专项 | router19/19；#69公共HTTP/Explain11/11；Grok receiver1/1 |
| 较早专项 | P0 router12/12、HTTP4/4；管理HTTP1/1；publish/rollback1/1；旧payload1/1；完整count5/5（最终Full再次涵盖这些测试） |
| 并行度 | `CARGO_BUILD_JOBS=2`、`RUST_TEST_THREADS=2`；独立 target/TMPDIR/数据库/日志/动态端口 |

工具链 `rustc 1.97.1 (8bab26f4f 2026-07-14)`、`cargo 1.97.1 (c980f4866 2026-06-30)`。
实际 serve binary SHA-256 `a5dcf0cffaae0b0083821640dfac0a5644ebc53126c1c21a81c5496010008ce6`；141214424 bytes。
归档在 `output/issue69/bin/gateway-46c2807eacb4`；binary 内三个 release marker 均为
`development`。身份由受测源码/lock/文件hash/构建命令绑定，不把该 binary 称为 release。
最终文档后继只改变 docs，受测 Rust/脚本/lock 未改变；最终文档 SHA 见交付消息和外层本票结果。

- [机器证据绑定](issue-69-controls-model-names-evidence/gate-binding.json)：源码、lock、binary、实际执行的测试binary、输入文件与日志hash。
- [最终Full明细](issue-69-controls-model-names-evidence/full-03.md)及[运行回执](issue-69-controls-model-names-evidence/full-03-receipt.json)。
- [首轮审查](issue-69-controls-model-names-evidence/review-initial.json)、[最终范围复核及修复](issue-69-controls-model-names-evidence/final-review.json)。
- [专项和RED日志回执](issue-69-controls-model-names-evidence/targeted-log-receipts.json)。完整日志与每个tracked输入hash保留于本票 `output/issue69/`。
- [Full01失败](issue-69-controls-model-names-evidence/full-01.md)、[Full02失败](issue-69-controls-model-names-evidence/full-02.md)及各自回执原样保留。

## 审查与保留失败

| 独立静态审查 | Standards | Spec | 修复/核验 |
|---|---|---|---|
| 首轮 5a837a04 | 3项P2 | 1项P1、2项P2（含1重复） | 5个独立问题修复，RED/编译失败分别保留 |
| 第二轮 3ed8ac01 | targeted recheck | count format/effort、Explain exact-ID 一致性问题 | 共享 admission 与 exact candidate 约束，新增直接回归 |
| 第三轮 65d83fa0 名称链路 | 指定范围无新可证实发现 | 指定范围无新可证实发现 | 无 Client Key 的管理视图边界保留 |
| 第三轮 65d83fa0 控制 wire | Grok 旧 oracle P1 | numeric suffix 范围 P2 | 46c2807e 修复，router/receiver 与最终 Full 核验 |

首轮固定 `5a837a04`：Standards 3项P2；Spec 1项P1和2项P2，重复1项，合计5个独立问题。
数字预算保真、坏 body 被 suffix 遮蔽、native output_config 额外字段、实际模型族判断、歧义 ID
被 Alias 加回均已修复。四项得到行为 RED，Alias 首轮只得到夹具编译错误，不能称为行为 RED；
后续 router17/17、歧义 Snapshot1/1、公共 receiver/count8/8通过。

第二轮静态复核指出 native count format 和已知模型不支持 effort 的 admission 差异，以及 Explain
未采用 exact-ID priority；已改成共享模型支持准入及管理全局 exact resolver，并补直接回归。
第三轮 `65d83fa0` 名称复核两轴未发现新可证实问题；控制复核指出旧 Grok oracle 和 OpenAI 数字
后缀遗漏最小预算。新增回归先观察到 `(1)` 输出 minimal（应为 low）的实际 RED，再修复。
最后修复后 router19/19、#69公共HTTP/Explain11/11、Grok unmapped receiver1/1通过；完整结果见本次门禁。审查是限定范围静态结论，测试执行结果独立记录。

Full01 在 `3ed8ac01` 因 Clippy 缺少 Debug 字段失败。后续告警以类型/辅助函数和小型 Box 修复，
没有降低 lint、跳过门禁或放宽断言。
Full02 在 `65d83fa0` 的 Rust gateway suite 为333通过/1失败/5忽略：Grok4.5冻结后旧 oracle
仍期待 xhigh/none。该轮前后源码均未变化。修正为已知模型 high/low，并补 unmapped tier receiver
断言；未知模型原 ADR0126 的 xhigh/none 提取测试保留。
全部早期编译/红灯/失败日志留在本票 ignored output，成功重跑没有覆盖旧文件。

## 共享变化和兼容性

- ADR0130、BC-PROTOCOL017；ADR0126、BC-PROTOCOL015/016、BC-ROUTE002 的限定后继说明。
- OpenAPI AliasInput 增加可选 prefix；通过既有 model_aliases、管理配置验证/版本发布/回滚。
- 公共 HTTP、runtime 装配、管理 Explain/effective model list 与 Router Snapshot 采用一致的实际名称约束。
- CanonicalRequest 可选 model_thinking 以 Box 保存可信名称意图；缺省序列化省略，旧 bytes 不变；
  新名称 payload 对旧严格读取器不保证二进制回滚兼容。schema39 不变、无 migration、无 Cargo.lock 或依赖变化。
- 无前端、通用 payload 脚本或新路由策略；没有修改其他并行票工作区。

模型族与适配器不一致时，CPAR 对 numeric intent 应用冻结范围后转换 OpenAI effort；
CPA 的已知模型 OpenAI/Codex applier 对 ModeBudget 可保留已翻译 body。因此该范围保证是明确的 CPAR
差异，不称为这些分支的逐字节 CPA 等价；具体来源和规则见支持矩阵。

## 证据边界和未执行项

LOCAL controlled receiver/fixture suite 不等于真实 #62/#64 客户端、固定 Build/service、真实 Provider、
远端 CI、release 或部署；这些均 NOT_RUN，保留独立资格要求。费用 UNKNOWN，#61 cost admission
及 cost_in_usd_ticks 仍 NOT_ADMITTED。native 和 explicit Lossless 边界按支持矩阵保留。
Explain 是无 Client Key 上下文的管理全局视图，不证明某个 Key 的实际 grant。
count 共用名称、支持和权限检查，不构造推理请求、不创建 inference Usage/ledger/quota observation，
也不保证一次计数会预测生成后的账单。未 push、合并、远端评论、关闭工单或部署。
