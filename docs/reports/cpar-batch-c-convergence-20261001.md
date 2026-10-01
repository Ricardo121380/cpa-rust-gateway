# CPAR C 批次软件／协议收敛补充 — 2026-10-01

本轮可实施的软件切片本地 **PASS**；**C 全批仍 BLOCKED，规格整体完成仍 PARTIAL**。
已修复 Kiro 失败后吐出排队事件，以及 Grok 原生元数据／生命周期的已证实缺口；完成回归、
两轴评审与固定源码 Full。新构建真实验收条件和 #57 台账已归集。未验证组合保持未完成。

## 固定范围与版本

- 实际 checkout：`/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway`。
- 分支：`codex/prism-v4-delivery`；比较基线 `f027e0bfe880fa175f764fa58694fae79af8771c`。
- 实现提交：`9499db7806470903a3960cd9d77c10395c7a1f2b`、
  `c90656abbe06a69bc5abad845c14365a53ffaa67`、
  `2e43ea4bccd7c1253df7d5ceddfbc3f4e4bc61fb`。最终 Full 绑定最后一个源码 SHA。
- 输入：[规格 #9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9)、#39–50 和
  [归集 #57](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57)；本目录资产中的只读 JSON
  保存读取时的 OPEN 状态与正文。冻结规格 379 行与读取的 #9 正文一致。
- 本轮没有 `web/prism/**` 业务修改、部署、服务启停、配置发布、账号/权限变更或远程工单写入。
  既有用户 AGENTS.md 修改和无关未跟踪文件保留；Full 时实现源码与候选一致，未声称整个工作区干净。

## 实际改变

Kiro 事件源在错误时清除待发队列并进入 Finished。开始前仍返回错误；开始后只发一个错误
终态，后续 poll 不再发文本、工具、ResponseStart 或成功终态。回归把有效帧与坏 CRC 帧放在
同一 chunk，分别覆盖响应开始前与开始后，并核对仅一次发送。

Grok Official/Console 接入既有 Canonical 原生 item/part 表示，保留 item 身份和顺序、
reasoning summary/content 分层、多 part、message phase、完整合法 citations、最终 null/空
logprobs，以及显式 Responses assistant/tool 历史回放。Build 复用封闭验证和关联辅助，
保留自身请求 profile 与 encrypted reasoning owner。移除运行工厂对未请求 reasoning 的静默过滤；
合法返回的 reasoning 仍保留，请求参数继续决定原生提交控制。

索引、工具 ID/name、完成快照与终态 output 必须确认先前观察内容。非空 logprobs、未审定
签名、未知 semantic event 字段仍失败；Official/Console 非 null ciphertext 仍不能透传。
Responses 可保留本轮审定富元数据；Chat/Messages 只允许单一普通 reasoning body 的等价桥接，
不能表示的富元数据仍正确终止。这不是新增已接受例外，相关模型/协议/参数组合仍需保留受阻。

评审又找出六类实际缺口，均已修复：完成 part 重开、done 后追加、生命周期字段/终态绕过、
初始引用丢失、完成快照逃逸 annotation 预算、item 完成后晚到同值 done。统一预算计算被保留
的 annotation 观察记录及 part/item 快照副本；不以只计增量事件绕过上限。合法分层完成确认保留。

决策记录为 [ADR-0101](../adr/ADR-0101-grok-native-item-metadata-fidelity.md)，并同步
BC-PROVIDER-008/013/015 和 traceability。ADR-0057 只补后续决策链接，未覆盖既定 Decision。

## 回归与门禁

| 检查 | 实际结果 | 证据和限制 |
|---|---|---|
| 初始 Kiro / Grok 反例 | Kiro 错误后发出 ResponseStart；Grok 元数据拒绝及 event-only 引用丢失实际失败 | [kiro-red.log](assets/cpar-batch-c-convergence-20261001/kiro-red.log)、[grok-red.log](assets/cpar-batch-c-convergence-20261001/grok-red.log)；实现前工作树加测试覆盖的诊断证据，不作为不可变候选门禁。 |
| 首轮实际反例 | 五类新增测试 FAIL，修复后转绿 | [review-red.log](assets/cpar-batch-c-convergence-20261001/review-red.log)；生产基线 `9499db78` 加测试覆盖。首次失败处停止，不冒称所有分支均红灯。 |
| 晚到 done 实际反例 | 1 个测试 FAIL，累计列出 7 条被接受路径 | [review-late-red.log](assets/cpar-batch-c-convergence-20261001/review-late-red.log)；生产基线 `c90656ab`。 |
| 最终 Grok 元数据专项 | **12/12 PASS** | [review-green.log](assets/cpar-batch-c-convergence-20261001/review-green.log)；Official/Console 原生 JSON/SSE、Build 原生 SSE；1/7/4096 字节分块、公共 JSON/SSE 编解码和历史回放、引用预算与生命周期正反例。 |
| 公开 runtime factory | **7/7 PASS** | [public-factory.log](assets/cpar-batch-c-convergence-20261001/public-factory.log)；实际 Actix/runtime/lease/attempt 路径接受控上游，不是只调用 builder。 |
| 前置受影响 crate 回归 | 57 条测试套件汇总，**522 PASS、0 FAIL、5 ignored** | [focused-regression.log](assets/cpar-batch-c-convergence-20261001/focused-regression.log)；在最终 helper 提取和晚到 done 修正前执行的补充证据，不能替代最终候选门禁。 |
| 最终严格 Clippy | **PASS** | [clippy.log](assets/cpar-batch-c-convergence-20261001/clippy.log)；相关五 package、all-targets/all-features、`-D warnings`。 |
| Standards | **PASS**，保留启发式 advisory | `c90656ab` 全 diff 复审＋`2e43ea4b` 增量复审；见 [review](assets/cpar-batch-c-convergence-20261001/review.md)。 |
| Spec | **PASS for correction**；C 完成 **PARTIAL/BLOCKED** | 第二轮唯一剩余补丁反例经实际复现、修复与第三轮增量核验；未声称声明扩展全量已审/已验收。 |
| 最终 Full | **44/44 PASS，退出 0** | [full-check.md](assets/cpar-batch-c-convergence-20261001/full-check.md)、[Full log](assets/cpar-batch-c-convergence-20261001/full.log)、[版本回执](assets/cpar-batch-c-convergence-20261001/full-run-receipt.json)；源码 `2e43ea4b`，2026-10-01 12:43:57–12:48:11 UTC，Darwin arm64。 |
| 最终证据静态门禁 | **7/7 PASS，退出 0** | [docs-check.md](assets/cpar-batch-c-convergence-20261001/docs-check.md)；证据文件另核对矩阵计数、状态、null 身份、校验值和 staged diff。工具日志仅规范化行尾空白/文件末尾空行，原始输出摘要保留在版本回执。 |
| 产品 EgoLite | **NOT_RUN，不适用本协议切片** | 没有本轮 UI 业务改动；Full 的 SPA 自动检查不等于浏览器产品验收。 |
| 远程 CI／release 产物／部署／生产 | **NOT_RUN** | 本地门禁不能替代这些层次。 |
| 新构建真实 Provider | **NOT_RUN，调用数 0** | 运行候选身份和实际配置/账号尚未重新核实；软件受阻组合另为 BLOCKED。 |

Full 已包含 Fast 的全部公共步骤及供应链三步，没有为同一源码重复启动 Fast。
Full 日志的 124 条 Rust 结果汇总为 1,446 PASS、0 FAIL、12 ignored；包含工作区既有测试与
doc tests，不能解释成 1,446 个新增场景或真实 Provider 组合。

公开工厂循环按源码断言计算为 **310 个成功公共请求、35 个负例**，不是独立真实请求回执。
成功数包括原 298 个请求及新增三 Grok 渠道 × JSON/SSE × 首次/显式历史回放的 12 个请求。
负例由原 15 个替换 Kiro 上限 8→16，再新增 12 个富元数据桥接负例得到；每类错误仍核对
唯一错误终态、无成功完成及不重发。Kiro 正例只覆盖 uncapped Chat/Responses；带上限的
16 个拒绝断言要求零新增 attempt/发送，不能证明 hard-cap 能力。Web 仍只有严格拒绝边界。

原工具工厂用例覆盖两个工具周期，但它们没有单独证明所有纯文本多轮、完整 Usage 账本、
全部 credential/地区组合或每项声明扩展。相关真实矩阵继续保留。

## 新构建验收准备与未完成组合

[real-channel-preconditions.md](assets/cpar-batch-c-convergence-20261001/real-channel-preconditions.md)
逐渠道列出新产物/实际进程 SHA、校验值、Config revision、精确 endpoint/route/account、
credential revision、出口/会话与能力冻结条件，以及原生输出上限、第二工具周期、Usage、
attempt 和扩展 oracle。旧 smoke runner 的运行 SHA 是调用者输入、只有一次工具回传，
不能只换成 LIVE_NEW_BUILD 标签就获得验收。

[combination-ledger.json](assets/cpar-batch-c-convergence-20261001/combination-ledger.json) 保存：

- **198** 个基础规划格：11 渠道 × 3 协议 × JSON/SSE × 独立多轮文本/单工具回传/连续工具。
  其中 **12 BLOCKED、186 NOT_RUN**；NOT_RUN 含 **12 个 Web 工具既定限制格**，其 applicable=false，
  不算正例通过。其余 186 个适用基础格全部仍无新构建真实通过证据。
- 另列 **24** 个 Kiro 显式输出上限组合（含 Chat 两个别名），均为软件 BLOCKED；Chat/Responses 未提交上限的
  基础规划格仍可独立准备，不能由它们关闭必填上限的 Messages 或 capped 组合。
- 另列 **12** 个条件性 Grok 富元数据 Chat/Messages 桥接格，保持 BLOCKED；实际模型/参数
  待冻结，单一普通 reasoning 正例不被这类条件性阻断覆盖。
- **49** 个逐渠道扩展准备条目。工单要求与按实际声明适用的条目分开，actual_declared 保持
  null；真实配置读取后须展开具体协议/模式/模型/参数并补齐任何新增声明，不能以 null 当未声明。
  Web continuity/WS 多轮条目仍软件 BLOCKED，其余未执行。

**Kiro 硬上限仍 BLOCKED。** 当前没有核实的 IDE/CLI 原生字段、单位与实际截断终态契约；
不能删除上限、改成提示词预算或只截断下游。Messages 必填 max_tokens 继续保留。

**Web 原生多轮及 Messages 必填上限仍 BLOCKED。** 生产 builder 仍要求单个 user，沿用
temporary new conversation。已有状态类型未接入公共可信 conversation/parent/account/revision/
egress 续接。原生参考只定位到线索，未完整核验；未靠取最后 user 或串接角色标签绕过。

**其他真实/扩展未完成。** 实际账号与运行配置本轮未刷新。2026-09-30 的 `b0cf36e`
旧运行观察仅见 [旧 C 报告](cpar-batch-c-20260930.md)，不是今天状态，也不是新构建验收。
不能用别的 endpoint/账号/合成 peer 的成功替代原目标，更不能自动关闭未知组合。

只有既定的 Web 工具不支持和精确 Messages token counting 不支持是已接受限制；其他缺口
保持软件阻断或未验证，不修改规格范围。

## #57 归集与回退

[issue-57-evidence.md](assets/cpar-batch-c-convergence-20261001/issue-57-evidence.md) 给出本轮
可审查归集说明，[evidence-manifest.json](assets/cpar-batch-c-convergence-20261001/evidence-manifest.json)
绑定输入和证据校验值。只归集 C 本轮切片，不重新接受全部 88 故事/F01–F08/T01–T15、
其他批次或数据兼容。#39–50、#57、父规格 #9 保持读取时 OPEN；没有远程评论、关闭或推送。

没有新增依赖、数据库 schema 或运行配置迁移。若后续决定回退，应按逆序撤销三个实现提交，
对回退源码重新运行相关门禁，并重新核对包含新增原生元数据的历史处理；本轮没有执行回退。
候选发布、实际账号恢复/route 发布、部署与生产验收继续遵守原有具体目标及授权边界。
