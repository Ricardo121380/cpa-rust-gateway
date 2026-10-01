# CPAR 执行身份、有效能力证据与 Linux ARM64 候选

日期：2026-10-02，Asia/Shanghai。**本轮软件切片、隔离回归与本地 ARM64 候选准备 PASS；生产部署、真实验收、远程 CI/签名发布及生产状态副本演练 NOT_RUN。C 全批、#57、父规格 #9 保持未完成。**

## 基线、提交与范围

以[上一轮报告](cpar-new-build-acceptance-20261001.md)及[真实前置条件](assets/cpar-new-build-acceptance-20261001/real-channel-preconditions.md)为基线，核对当前入口、计划、回执与 GitHub #9、#39–#50、#57。执行开始 HEAD 为 `09e70e2aa23a8541e8e20726edb15bdc96a6c2af`，上轮正式工具源码 `ff4b59edda6cd480782e036dbb7cecee762873be` 的证据语义不变。

本轮本地提交：

| 提交 | 内容 |
| --- | --- |
| `ff77a5f05dc3145035cf14c4c2833f24833cafd2` | 从实际 driver/lease/配置/transport 捕获执行身份；schema34 原子证据；受保护能力声明/pin；正式入口 freeze/采集/严格核对；回归与契约同步 |
| `c94b35edae148445c3c3cd44edc784807fe45762` | 两轴评审确定问题修复：Web relay 不误认备用 proxy；采集器严格拒绝非规范证据 |
| `179cf325ee9f9c0531a6d343467021928c1076c0` | Full 发现的既有全表清单遗漏，加入 execution sidecar |
| `80f5838902eb56be6dccc3ce047eb227d6c18e6c` | Full 发现的 composition-root `sha2` 依赖清单遗漏，登记及解释边界 |

**正式验证和候选固定源码为 `80f5838902eb56be6dccc3ce047eb227d6c18e6c`。** 后续证据交付提交仅保存报告、计划、回执和构建配方，不改变候选源码。候选由独立完整 clone 的 clean tracked checkout 构建；用户已有 `AGENTS.md` 修改及无关未跟踪文件未进入提交或制品。没有 push、远程评论或关单。

## 实际执行身份与读取链

最小实现复用既有 Request/Attempt/Usage/账本，补上有界 `AttemptExecutionIdentity`。实际渠道、endpoint、route/candidate、上游模型、credential ID、request/attempt ID 和结果复用原 Attempt/Request；新增字段从执行位置读取实际 `CredentialLease` revision、driver 钉住的配置 version/revision、实际 egress 和本候选编译能力。

- 每次选定候选和 lease 后创建该 attempt 的事实；配置发布、材料轮换与换候选不会重写旧请求。管理账号 revision、native row revision、实际 lease revision 分开保留，不互相代用。
- 配置图版本来自实际加载 generation；egress 来自实际选择的 transport / configured node。Direct 的 revision 为“不适用”；配置所有的 pool/node 使用同一加载 config version/revision；进程 proxy 使用校验后 secret-free transport 的 SHA256 指纹。Web relay 的 onward 出口无法由 gateway 确认，明确 Unknown。
- `gateway_attempt_execution` 与原 Attempt 同事务追加，以 request ID、attempt ID 和原严格 payload SHA256 绑定。原事件 JSON/旧严格解码格式保持原样。相同重放必须同时匹配原事件和 sidecar，不能用新 inventory 给旧事件补证。
- 受保护 pin 的可选 `execution` 返回实际单次 attempt；普通请求仍经现有 Usage 与 exact source-event 账本归属。Pin 沿用诊断路径，不制造重复计费事件。
- 原 `collect` 在一致只读 SQLite transaction 中读取 Request、terminal、Attempt/sidecar、Usage 和 exact ledger source；检查实际 Client Key/protocol/mode/outcome及每个冻结目标。没有从管理 inventory 倒填历史请求。
- 观察仅包含闭合字段、必要 ID、revision、能力和校验值；ID 有界，sidecar 最大 8192 字节。Debug/公开回执不包含正文、令牌、密钥或原始 proxy URL。

[八个实际隔离记录样本](assets/cpar-execution-evidence-20261002/execution-examples.json)来自已结束且 checkpoint 的候选 fixture：三协议×JSON/SSE 六条原始归属，加材料/config/node 切换前后两条。它们精确关联 request/attempt、原 payload hash、Usage source 和账本。样本层为 LOCAL_SYNTHETIC_HISTORICAL_FIXTURE_EXPORT，不伪装成当前生产执行。完整正式采集器链已由同一候选的 HTTP 回归验证。

## 有效能力证据与验收入口

`/admin/models/effective` 的每个可见 source 可选提供 `capability_evidence`：仅当 management projection 对应实际 serving generation，给出 `serving_snapshot`、加载配置 version/revision，以及 reasoning、parallel、stored、continuation、compact、WebSocket 六项编译声明。访问范围继续使用当前 Client Key/Access Group graph，projection fingerprint 与 candidate/model 精确绑定。

正式入口新增无发送的 `freeze`，只从已有成功、fresh、single-attempt、相同构建和 Client Key 的 pin，以及当前匹配的 capability projection 填充未知字段。`run` 发送前核对声明，发送后逐响应核对实际 Request/Attempt/Usage/ledger；未知则 BLOCKED，矛盾则 FAIL，停止下一轮。没有通过关闭能力、改权限、改配置或降低声明取得通过。

空 public capabilities、缺 override 或 null 不视为 false。三协议适用性另行评估，Responses-only 的 stored/continuation/compact/WebSocket 在 Chat/Messages 不适用时仍保留原 compiled declaration。Grok Build OwnedReasoning continuation 沿用既有规则，要求 Reasoning；不扩展为新的 Provider 重写。

当前生产仍运行旧构建；[最新只读观察](assets/cpar-execution-evidence-20261002/current-environment-final.json)中的七项 capability source 均未知，actual lease、loaded integer config revision、actual egress 也不能由旧管理库存证明。两个新[首批草案](assets/cpar-execution-evidence-20261002/release-and-first-acceptance-plan.md)已补全可可靠取得的 route/candidate/model、源码和 public response model；执行 revision、出口和能力 proof 仍 null，等待获准部署后的实际 pin/GET。管理 `rev-1`、native revision 和 public `{}` 没有倒填到这些字段。

## 契约、迁移和回退兼容

- 新增[ADR-0102](../adr/ADR-0102-attempt-execution-identity-and-capability-evidence.md)、[BC-OBS-005](../contracts/BC-OBS-005-attempt-execution-identity.md)，更新 traceability、contract tests 和 ADR 索引。
- OpenAPI 对 pin execution / effective source evidence 为可选、闭合、受保护扩展。已运行 `npm --prefix web/prism run sync-contract`；vendored contract 与 authority SHA256 相同，通用生成客户端没有文本变化。契约和镜像、同提交跨边界记录及 trailers 已交付；未修改前端业务。
- schema 34 只新增 append-only sidecar，FK 指向原 Attempt；旧记录/缺失字段继续 Unknown。破坏绑定、错误 revision、重复键、非规范编码、数值溢出和未知字段拒绝读取。
- 原子失败回滚、精确重放、legacy schema33→34、34 down→33→34 和旧原事件严格解码已有回归。down 会丢弃 sidecar、保留原事件，重新 up 不自动补历史；不能用此行为作为生产丢证例外。
- 实际 ARM64 候选把含 847 个事件、184 条账本的合成 schema31 状态启动迁移到34；逐行比较14张表的原有列，均保持；新增 legacy usage provenance/input accounting 为 Unknown；旧 attempts sidecar=0。完整副本恢复为31、quick_check和全部原快照精确一致。[兼容回执](assets/cpar-execution-evidence-20261002/native-compatibility.json)。
- 合成 native Grok 表为空；生产 native 数据演练和旧 ELF 启动恢复副本 NOT_RUN。旧二进制拒绝新 schema，发布回退必须恢复匹配的完整 pre-upgrade 状态与 keys，再切换旧二进制，不能只换 symlink。

## 回归、评审与门槛

| 验证 | 固定对象 | 结果 / 证据 |
| --- | --- | --- |
| 原基线缺口复现 | 原 HEAD + 定向 fixture | 六条响应能成功但无 sidecar，[RED](assets/cpar-execution-evidence-20261002/red-http/receipt.json) |
| 正式入口回归 | 最终源码80f58389 | 30/30；三协议JSON/SSE、未知/矛盾零后续发送、精确revision/source/ledger、legacy/tamper、泄漏负例 |
| 真实 gateway + SQLite + TLS + 两个实际 SOCKS relay | 最终 Linux ARM64 release binary | **410/410 PASS**；三协议JSON/SSE归属，错误/取消/超时，候选重试，材料/config/node变化，在途隔离，protected pin与正式collector链，[回执](assets/cpar-execution-evidence-20261002/native-http/receipt.json) |
| Native factory / runtime / store 回归 | 最终源码80f58389，Full | 现有 native渠道JSON/SSE增加实际身份断言；lease/config冻结、process proxy指纹、relay Unknown、OwnedReasoning规则、原子/严格读/迁移回退全部通过 |
| Standards / Spec 两轴 | 固定初评、修复diff；最终两处机械补齐由主代理审查 | 两项P2先RED再修复；本切片最终两轴PASS，[范围与关闭证据](assets/cpar-execution-evidence-20261002/review.md) |
| Full | 最终源码80f58389，macOS ARM64 | **45/45 PASS**，包含format、strict Clippy、Rust全工作区、SPA/契约、crate boundaries、secret scan、deny/audit；[报告](assets/cpar-execution-evidence-20261002/full-gate.md)、[绑定回执](assets/cpar-execution-evidence-20261002/full-run-receipt.json)、[日志](assets/cpar-execution-evidence-20261002/full-gate.log) |
| Release artifact / ELF / OCI / startup / state | 最终源码80f58389，native Linux ARM64 | 本地 unsigned 结构验证与启动/兼容 PASS，[候选回执](assets/cpar-execution-evidence-20261002/candidate-receipt.json) |
| Remote CI / Sigstore / production-copy / deploy / real Provider | 本轮范围之外待具体授权与前置条件 | **NOT_RUN，real_provider_calls=0** |

前两次 Full 分别揭示“全表断言未登记新增表”和“composition-root 已用依赖未登记”；[第一轮](assets/cpar-execution-evidence-20261002/full-gate-first.md)、[第二轮](assets/cpar-execution-evidence-20261002/full-gate-second.md)及原输出保留。修复后最终45/45，而不是将失败行忽略。早期构建包装脚本的 `--version` 不支持、Git目录属主检查及 pipefail/SIGPIPE 也保留日志，最终成功构建使用修正后的隔离脚本。

## ARM64 候选及验证

候选位于：`/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway/output/cpar-execution-evidence-20261002/linux-arm64/80f5838902eb56be6dccc3ce047eb227d6c18e6c/`。包含二进制、OCI tar、SBOM、build metadata、预期 signing identity 与 manifest；大型制品留在该独立目录，manifest/回执/日志进入本地 Git。既有 output 内容保留。

| 文件 | SHA256 |
| --- | --- |
| gateway-aarch64-unknown-linux-gnu | `bd2e46ed6ab7174e2cfcdebe88ca7cba8cc1fb0b0ff46e9714469bc900cdadc0` |
| gateway-image.oci.tar | `12e5119411a919dcb39bfb756bf23c421bf474be13d7cc0d12d681a015055c52` |
| gateway-sbom.cdx.json | `7dc683785a261ba33f5f66dd34093cf2c87a89211ed4ac711b53b9266b734947` |
| artifact-manifest.json，文件字节 | `4cb4eac4e787eba6fe944b31f258f12af380a7179888a3216d75251aa00c90f7` |

正式入口对解析后的 manifest 使用 canonical JSON digest，值为 `9ae09be6643d1a5427c795f57d2f291d403c1544faf8feb84094501a4d8f4be7`，与文件字节 hash 的口径不同；payload hash 完全一致。

构建采用项目 `build-release.sh`、P12 metadata/manifest/verifier、SBOM 脚本和原 Dockerfile，环境为独立 Colima native aarch64 Linux / Debian bookworm，Rust/cargo 1.97.1、Node24.19.0、glibc2.36。Rust/Node/OCI base 均使用记录的 image digest；apt 版本与 linker/compiler 在[工具链记录](assets/cpar-execution-evidence-20261002/builder-toolchain.log)，[配方](assets/cpar-execution-evidence-20261002/candidate-build/README.md)可复核。没有宣称跨环境逐字节可重建。

验证原 ELF e_machine=183、嵌入 source/Rust/target与manifest一致；实际启动的受保护 `/admin/system` 返回 version0.1.0、同SHA、target及schema34。OCI 默认65532:65532、read-only、network none运行 `--help` 成功；候选实际服务启动由410回归和31→34兼容验证覆盖。最大GLIBC符号要求2.34，当前目标2.39；目标动态库和真实状态仍需发布前演练。

## 指定目标发布与首批验收

[完整方案](assets/cpar-execution-evidence-20261002/release-and-first-acceptance-plan.md)固定 old→candidate 版本、路径/hash、签名前置、目标动态库检查、一致离线备份、一次该服务stop/start、迁移核验、明确停止条件和完整回退。现有服务、Caddy、活跃配置、账号及权限未变更。

首批拟 Krill `gpt-6-sol` Responses JSON，再 Grok Build `grok-4.5` Responses JSON。每个组合拟 1次受保护pin+7次basics，最多8次推理；这是待批准预算，不是既有真实请求授权。实际执行前重新核对账号、Client Key、配置、出口及 scoped能力，pin/freeze/正式预检均成功后才加入 `--execute`。当前旧 runtime 对候选[预检](assets/cpar-execution-evidence-20261002/current-runtime-preflight.json)为预期 FAIL，未发送请求。

Kiro输出上限、Grok Web原生多轮及上限继续独立 BLOCKED；未知relay onward同样阻断该组合。未进行大型Provider重写，未新增例外，未降低能力声明。

## #57 本地归集与剩余工作

[归集清单](assets/cpar-execution-evidence-20261002/issue-57-evidence.md)和[结构化状态](assets/cpar-execution-evidence-20261002/issue-57-evidence.json)分开记录：本轮软件观测切片PASS，本地候选PASS，真实新构建/生产验收NOT_RUN，#57/C/父#9未完成。

剩余项为：获准远程签名发布与校验；生产native状态副本迁移/恢复和目标动态库演练；指定服务有界部署与版本核验；实际pin/能力冻结；三协议JSON/SSE真实basics；声明的扩展及受控边界样本；原Kiro/Grok Web独立问题；逐项C/#57/#9验收条件。没有因候选准备而关闭任何一项。

证据交付阶段静态检查[7/7 PASS](assets/cpar-execution-evidence-20261002/delivery-docs-check.md)，新报告/回执秘密扫描及staged diff whitespace通过。临时本地构建资源已[收尾恢复](assets/cpar-execution-evidence-20261002/local-environment-restoration.json)；生产环境仅只读观察。
