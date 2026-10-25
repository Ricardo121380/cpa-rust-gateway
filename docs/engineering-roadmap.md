# CPAR 当前状态与开发顺序

更新：2026-10-24（Asia/Shanghai）。本页是**当前功能状态、CPA 对照版本和下一步开发的唯一汇总入口**。
[架构文档](03-target-architecture-draft.md)负责代码位置，[支持清单](support/first-release-candidates.md)负责客户端与交付证据，
[GitHub 工单](https://github.com/Ricardo121380/cpa-rust-gateway/issues)负责具体验收条件及远端状态。
旧计划的“待定”“未实现”和历史 PASS 均按原日期理解；后续实现与证据在这里汇总。

## 1. 产品方向与当前基线

CPAR 是 CPA/CLIProxyAPI 网关行为的 Rust 实现。优先直接对照固定的 Go 实现，再修改对应 Rust 模块。
已有 Grok Web/Console、Kiro、持久化管理与账本继续保留。旧 Prism 已退役，新前端以后依据后端 API 重新设计。
现阶段继续完善已有渠道和 Codex、Claude Code、Pi 的调用闭环；新增 Provider、多模态、插件等按具体需求另排。

| 基线 | 当前身份及用途 |
|---|---|
| 本次盘点源码 | `feature/62-client-metadata`，HEAD `ab78c65ca68978bf46c7cf530ea795417d8431f3` **加本地未提交修改**；包括后端整理、前端退役、ADR-0125/0126 和 Tokio1.53.3安全修复 |
| 实际检查目录 | `worktrees/cpar62-client-metadata-20261020`；不是把主克隆路径当作当前 checkout |
| 当前功能对照及后续移植参考 | [CPA v8.0.40](https://github.com/router-for-me/CLIProxyAPI/tree/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683)，commit `d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683` |
| 上游核对时间 | 2026-10-23 14:48:53 UTC；当时最新 release tag 与 main 一致。固定此提交，不追随浮动 main 改变预期 |
| 已有实现来源 | ADR-0125 等仍基于 v7.2.158；ADR-0126 的 Messages effort 提取基于 v8.0.40。既有差分样例保留 v7.2.80/v7.2.101 的来源，不宣称整体 v8 对齐 |
| 功能证据 | [10 月 23 日 CPA v8 对照报告](reports/cpa-v8-functional-comparison-20261023.md)；后续 effort 增量和新源码验证见 [10 月 24 日实施记录](reports/cpa-v8-messages-effort-20261024.md) |
| GitHub 状态核对 | 2026-10-24（Asia/Shanghai）；已修订22张开放票、创建 #68–#75，并独立读回正文、状态和原生关系 |
| 本轮交付边界 | 在已有 effort/Tokio 修复之上完成规格与工单发布；本轮没有新增业务实现、行为测试、实际客户端、固定服务、Provider、软件发布或部署结果 |

更新移植参考只决定后续查哪份 Go 代码，不覆盖已接受 ADR 的具体行为，也不重标旧测试、署名和收据。
此前 effort 增量通过 ADR-0126 限定后继修订记录行为变化；ADR-0125 原始来源和历史回执保留。

## 2. 当前功能状态

“接入生产”表示当前正式构建的调用链有实现，不表示已在真实 Provider 或线上环境验收。
实现、装配和验证分别记录；功能没有进入正式路径时，写清缺的是接线、启用条件还是实现。

| 功能 | 当前代码与装配 | 验证边界 / 下一动作 |
|---|---|---|
| Chat / Responses / Messages，JSON/SSE、常规工具 | 已实现并接入公共 handler 与 runtime | 按所选渠道/模式限制；当前候选真实客户端结果另验，不重开基础协议开发 |
| Claude adaptive/effort/cache 兼容 | ADR-0125 已接入 native / CanonicalBridge；ADR-0126 已补齐 v8 显式 effort 提取及优先级 | effort-only、enabled 无预算、预算/disabled 优先级和 native JSON/SSE receiver 已验证；模型默认/预算输入范围仍有差异，最新源码实际客户端待重跑 |
| Messages count_tokens | 路由与可注入 executor 已有；生产仍装配 UnsupportedCountTokensExecutor；ADR-0127 已接受适用上游计数与明确标识估算 | **产品决定已确认，待实现与接线**；计数不进入实际 Usage/账本，不能列为已可用 |
| namespace、hosted search、部分 input IDs/history、Build parallel | codec/受控测试已有；适用请求的生产资格分支仍为 false | **待满足明确资格并启用**，不重复造 parser，也不直接删除资格检查 |
| Responses 查询/删除、续接、compact | 已实现归属保护和存储；StoredResponses 限 Build/Web，compact 限 Build | 按渠道及精确 lineage 验收，不推广到全部 adapter |
| 入站 Responses WebSocket | 已实现并桥接现有 HTTP/SSE 出站 | 原生上游 WS、Realtime 尚未实现；已有入站接口不重复开发 |
| Claude/Codex/Kimi 账号 | 授权、导入、刷新与兼容 adapter 推理路径已有 | 具体账号/渠道真实调用另验；不是旧矩阵里笼统的“未来 OAuth” |
| Grok 官方/Build/Web/Console、Kiro | 专用 adapter 与账号管理已有 | Web 当前文本流式、工具不支持等限制保留；不能据注册数宣称全部组合已验收 |
| 模型发现和目录 | Build、官方 Codex、Kiro，以及配置 models_path 的兼容 endpoint 已有 runtime source | 其余渠道未因此自动获得 discovery；模型发现不自动授予 Key 权限 |
| 账号池、路由、重试和权限组 | 已接入；普通模型入口使用 ProviderScopedSelector | 与 CPA priority/weight/fill-first 行为不同；需要行为对齐时按实际 serving 分支改 |
| 通用 session/cache affinity | 精确 continuation pin 已有；Build cache_affinity 库/测试未发现 serving 接线 | **待接线/补策略**，不能把两者合并报为通用亲和已完成 |
| 管理 API、配置发布/回滚 | /admin 已有；管理 API 发布时装配新运行代 | 本地离线 admin CLI 与运行中管理 API 的生效方式分开；无需重建控制面 |
| 账号配额观察历史（#74） | 独立历史表、现有额度读取/Kimi后台观察接线、有界管理查询已实现 | 本地验证见[交付记录](reports/issue-74-quota-history-20261025.md)；真实Provider、部署未运行；与Token Usage/费用分离 |
| Request/Attempt/Usage、账本和加密恢复 | 持久化与查询/恢复实现已有 | 未知费用、cost_in_usd_ticks 的未准入状态保持；长期统计工作流仍有差距 |
| 外部 OTel 输出 | 接口已有，serve 使用 Noop exporter | **待接线**，不宣称已把 tracing 送到外部系统 |
| 通用 payload 覆盖、自定义 headers、模型前缀/后缀等 | 未发现 CPA 等价用户配置入口 | **待实现**，按实际兼容需求排序 |
| HTTP/HTTPS/SOCKS5h/认证代理 | 已有直连/固定/池化出口；当前只接纳限定 SOCKS5 | **范围差异**，后续扩展须保持明确的出站约束 |
| Gemini 等新增 Provider、图片/音频/视频、Realtime | 当前原生注册/公共路径未发现 | **未实现、尚未列入近期批次**，保留需求选择空间 |
| 动态插件、Home、多存储后端、对外 SDK/TUI | 未发现 CPA 等价产品功能 | **未实现、按需后置** |
| 旧 Prism / 新前端 | 旧源码与嵌入构建已按 ADR-0124 退役；管理 API 保留 | **旧项已退役，新项待设计**；不继续排旧 Prism 修补任务 |

主要代码入口：

- [公共路由与默认计数器](../crates/gateway-http-actix/src/lib.rs)、[生产计数器装配](../apps/gateway/src/runtime/reload.rs)。
- [runtime 注册、能力、资格、catalog source 与 OTel 装配](../apps/gateway/src/runtime.rs)：
  `p12_api_format_adapter_registry`、`p12_adapter_capabilities`、`validate_native_field_candidate`、`RuntimeModelCatalogWorker::try_new`。
- [兼容投影](../crates/gateway-router/src/protocol_transform/compatibility.rs)、[实际路由排序](../crates/gateway-router/src/provider_scoped_selector.rs)、
  [Build 亲和库](../crates/provider-grok/src/continuity_state.rs)、[常规 OAuth 刷新](../apps/gateway/src/credential_refresh/ordinary.rs)。
- [配置运行代接线](../apps/gateway/src/deployment.rs)、[原子发布](../crates/gateway-control/src/snapshot_publisher.rs)、
  [当前代理范围](../crates/gateway-upstream/src/upstream_client.rs)。

完整 CPA 差异和逐项出处使用[功能调研报告](reports/cpa-v8-functional-comparison-20261023.md)；
统计生态使用[CPA/CPAMP/Keeper 报告](reports/cpa-ecosystem-comparison-20261023.md)。
旧 [313 项功能选型矩阵](01-feature-selection-matrix.md)保留范围决定和 ID，不再用其“待定”计算当前实现进度。

## 3. 旧工作分解如何对应现在

[10 月 18 日路线图](reports/engineering-roadmap-20261018.md)中的 S1–S5 是当时的拆分，不能原样重新启动：

| 旧工作项 | 当前应如何理解 |
|---|---|
| S1 字段/行为契约 | 已有后续 ADR、局部字段及请求兼容实现；#61 适用字段和服务资格仍有缺口，不能说全部从零待做 |
| S2 标准 Responses | 公共执行、工具/历史及加密证据相关实现和受控记录已有；生产资格和真实支持范围仍有限 |
| S3 标准 Messages | 摘要完整加密留存、JSON/SSE、工具续轮和后续请求控制已有本地实现；不能继续写“摘要留存尚未实现” |
| S4 三个实际客户端 | 历史受测源码已有 Codex/Pi/Claude D2 受控结果；原始 Claude 旧拒绝不代表 ADR-0125 后仍必然拒绝；最新候选需重验 |
| S5 候选/交付 | 已有 binary/OCI 生命周期证据；同源三客户端、固定服务资格和真实验收仍未完成 |

这些对应关系只纠正开发状态，不抹掉旧失败和未勾选验收。
具体源码、版本、限制以[候选支持清单](support/first-release-candidates.md)和原报告为准；最近行为增量只补 effort 本地验证；本轮票务同步没有重跑这些历史客户端或制品资格。

## 4. 工单、生产资格与远端同步状态

以下状态于2026-10-24规格与票务发布后重新读取；正文、原生blocked-by与sub-issue关系均已核对。
OPEN不等于没有代码，CLOSED也不代表整个渠道已完成；本轮没有关闭旧票或解除既有阻塞。

| 工单 | 远端状态 | 当前使用方式 |
|---|---|---|
| [#59](https://github.com/Ricardo121380/cpa-rust-gateway/issues/59)、[#60](https://github.com/Ricardo121380/cpa-rust-gateway/issues/60) | CLOSED | 已关闭的诊断/映射切片，不重新排入待实现；原字段、别名及适用范围限制保留 |
| [#61](https://github.com/Ricardo121380/cpa-rust-gateway/issues/61) | OPEN | 适用字段/固定服务依据仍有限；cost_in_usd_ticks 为 NOT_ADMITTED |
| [#62](https://github.com/Ricardo121380/cpa-rust-gateway/issues/62)、[#63](https://github.com/Ricardo121380/cpa-rust-gateway/issues/63) | OPEN | Responses 客户端与 Messages 本地实现/受控证据已有；各自依赖和完整验收未解除 |
| [#64](https://github.com/Ricardo121380/cpa-rust-gateway/issues/64)、[#65](https://github.com/Ricardo121380/cpa-rust-gateway/issues/65) | OPEN | Claude D2 和进程恢复已有历史证据；整票条件及新源码验证继续保留 |
| [#66](https://github.com/Ricardo121380/cpa-rust-gateway/issues/66) | OPEN | 同源候选和真实验收前置；已有 OCI 生命周期 PASS 不代替目标制品上的三客户端执行 |
| [#67](https://github.com/Ricardo121380/cpa-rust-gateway/issues/67) | OPEN，未开始 | #66 条件满足后，按明确账号/环境/预算和新 epoch 执行真实 Grok Build 验收 |
| [#58](https://github.com/Ricardo121380/cpa-rust-gateway/issues/58) | OPEN | 首版 Codex、Claude D2、Pi 均通过 Grok Build 的产品范围保持，尚未 READY |
| [#9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9)、[#47](https://github.com/Ricardo121380/cpa-rust-gateway/issues/47)、[#57](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57) | OPEN | 现有渠道和全范围收口义务保留，不用局部兼容改动关闭总项 |

已核验的原生阻塞：`#59/#60 → #61 → #62/#63`，`#63 → #64/#65`，`#62/#64/#65 → #66 → #67`。
#66 不要求先通过 #67 的真实 Provider 验收，二者不存在相互等待的要求。

**远端规格与工单已同步（2026-10-24）：** 已修订 #9、#39–#50、#57、#58、#61–#67 共22张开放票，
纳入后端方向、ADR-0125/0126请求兼容及ADR-0127计数策略。#64的原始Claude阻断和旧effort预拒绝按历史受测源码解释，
当前行为与后续重验要求分别列出；原source、214项验收及勾选、依赖和资格边界保留。
本轮发布不新增运行证据，原版客户端和真实服务验证义务仍未完成。

[当前产品规格 #58](https://github.com/Ricardo121380/cpa-rust-gateway/issues/58)统一列出8张优化票；
#61–#67和#68–#75已关联为其15个原生子票。#9的既有48个子票关系保持，父子关系不等于阻塞。

## 5. 下一项与后续顺序

**已完成本地实现：对齐 CPA v8 的 Messages effort 提取和优先级。**

| 项目 | 当前结果 |
|---|---|
| 用户行为 | 独立 output_config.effort、enabled 无预算均保留显式强度；disabled 和正预算优先 |
| Go 来源 | [extractClaudeConfig](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/thinking/apply.go#L740) |
| Rust 实现 | [compatibility::thinking](../crates/gateway-router/src/protocol_transform/compatibility.rs)；沿用现有解析、路由和 builder |
| 行为决定 | [ADR-0126](adr/ADR-0126-cpa-v8-messages-effort.md)限定修订 ADR-0125；enabled 无有效 effort 两目标均为 auto |
| 本地验证 | 路由回归 9/9；公共 JSON/SSE 40 个跨协议和 10 个 native 正向请求、12 个零新增发送负例通过；完整门禁结果及源码身份见[实施记录](reports/cpa-v8-messages-effort-20261024.md) |
| 剩余边界 | 未新增模型专用 clamp；0/-1 预算和 malformed controls 仍拒绝；实际客户端、固定服务、发布另验 |

**质量门禁修复（2026-10-24）：** Tokio 最低版本与锁文件已更新为1.53.3，
RUSTSEC-2026-0386 阻碍已通过实际升级消除；cargo deny通过，独立 audit无漏洞或警告。
当前源码加依赖增量的完整门禁 **PASS 44/44**，Rust测试1671通过、0失败、21忽略；
源码绑定与结果见[安全维护记录](reports/tokio-security-fix-20261024.md)。
[effort 实施记录](reports/cpa-v8-messages-effort-20261024.md)保留原始 FAIL 和原受测源码，不改写历史结果。

**下一项建议：补齐 Messages count_tokens 的生产实现与装配。**
路由和注入接口已经存在，生产仍使用 UnsupportedCountTokensExecutor；从现有 executor/runtime
接缝对照 CPA 计数行为。用户已按 [ADR-0127](adr/ADR-0127-cpa-compatible-token-counting.md)
确认适用上游计数与明确标识的本地估算；实现时明确支持模型/输入、来源与估算局限，再完成生产验证。
本轮已按确认的8票合并版发布 #68–#75，75项验收条件全部保留。下一项对应 [#68](https://github.com/Ricardo121380/cpa-rust-gateway/issues/68)；工单发布不代表计数已实现。
其后按需要推进常用 Claude/Codex 兼容与当前客户端验证 → 会话亲和/路由配置便利性。
高级工具的生产资格与固定服务证据继续按各自条件推进。
Gemini、多模态、插件、Home 和新前端是单独扩展项，本页没有自动启动它们。

**已发布的优化队列：**

| 工单 | 交付目标 | 优先级 |
|---|---|---|
| [#68](https://github.com/Ricardo121380/cpa-rust-gateway/issues/68) | Messages 计数：上游计数与明确标识的本地估算 | P0 |
| [#69](https://github.com/Ricardo121380/cpa-rust-gateway/issues/69) | CPA 请求控制与模型命名兼容 | P0/P1 |
| [#70](https://github.com/Ricardo121380/cpa-rust-gateway/issues/70) | 生产路由策略与 Build 会话亲和 | P1 |
| [#71](https://github.com/Ricardo121380/cpa-rust-gateway/issues/71) | CPA 请求规则与自定义上游 headers | P1 |
| [#72](https://github.com/Ricardo121380/cpa-rust-gateway/issues/72) | Token 与费用的统一查询及快照导出 | P1 |
| [#73](https://github.com/Ricardo121380/cpa-rust-gateway/issues/73) | 价格获取、审阅与版本化应用 | P1 |
| [#74](https://github.com/Ricardo121380/cpa-rust-gateway/issues/74) | 配额观察的历史与重置窗口查询 | P1 |
| [#75](https://github.com/Ricardo121380/cpa-rust-gateway/issues/75) | 大历史查询与加密备份恢复的容量闭环 | P1 |

8张新增票均为OPEN、ready-for-agent，之间没有新增阻塞边；请求控制→模型命名和统计查询→导出按票内顺序实施。
#69的请求控制保留P0、模型命名保留P1；合并不增加首版门槛。原规格发布时未实施这些新增工单；#74后续本地增量见上表及交付记录，其余票按各自会话处理。

已接受的 native-v1 方向及原 N1–N4、Kiro cap、Grok Web 多轮/能力、#9/#47/#57 的剩余承诺保留。
具体历史范围见[归档路线图](reports/engineering-roadmap-20261018.md)及原工单；本次没有取消这些目标，
也没有重新使用历史真实请求预算。

## 6. 如何保持同步

每次相关实现或资格变化，只更新本页受影响的功能行和下一项，引用代码及有准确源码身份的证据；
README、AGENTS、文档索引保持短入口，不再复制一整份动态进度。

- **实现变化**：同时核对注册、生产装配和能力限制；库或测试存在不算已接入。
- **证据变化**：记录候选/source、客户端/环境和适用范围。历史报告保持原日期、失败、计数和身份。
- **上游变化**：显式更新本页固定版本，列出实际行为差异；旧实现来源和差分样例分别保留。
- **工单变化**：需要最新状态时读取 GitHub；没有远端写入时明确记录待同步，不把本地表当关闭工单。
- **范围变化**：已接受 ADR/契约按正式后继流程调整；旧选型“待定”不否定后续明确决定，也不授权顺手加功能。

## 7. 文档分工

| 文档 | 负责内容 |
|---|---|
| 本页 | 当前基线、功能实现/接线、阻塞、下一项 |
| [后端架构](03-target-architecture-draft.md) | Go/Rust 模块对应和修改入口 |
| [候选支持清单](support/first-release-candidates.md) | 客户端、固定服务、binary/OCI 与真实验收的证据身份 |
| [CPA v8 对照报告](reports/cpa-v8-functional-comparison-20261023.md) | 日期固定的详细调查与出处 |
| [历史路线图](reports/engineering-roadmap-20261018.md) | 10 月 18 日原目标/访谈/工作卡，含仍保留的扩展义务 |
| [旧参考基线](00-reference-baseline.md)、[选型矩阵](01-feature-selection-matrix.md)、[P0–P13 计划](06-development-plan.md) | 原版本、需求 ID、阶段与决定；不作为当前实现状态表 |
| [Traceability](traceability.md)、ADR、contracts | 需求—决策—实现—证据关系及适用约束 |
| reports/assets | 固定时间与源码的证据，按原身份读取 |

10 月 23 日入口整理保留为文档历史；10 月 24 日 effort 代码与契约变化已按 ADR-0126 登记。
本轮没有修改 AGENTS/CLAUDE，也没有改写旧失败、原请求计数或已保存的测试源码身份。
