# #71 本地实施：版本化请求规则与自定义上游 headers

日期：2026-10-25（Asia/Shanghai）。状态：PASS_LOCAL_IMPLEMENTATION。
仅覆盖下述明确支持范围；GitHub 工单保持远端原状态，没有 push、合并、评论、关闭或部署。

## 受测身份与固定输入

- worktree：`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar-71-request-rules-upstream-headers-20261025`。
- 分支：`feature/71-request-rules-upstream-headers`；起始基线 `7c16070bb66f7f3008022f7870b5d1f96b63f167`。
- 最终受测 commit：`2dc8c87c9c8f7c78db4c368d62932be5b4573e82`；tree：`1a1ea84e86e16bca9f1740d5dfe09c97819837c1`。
- Cargo.lock SHA-256：`3b0c82a72bea2fba0b9de42ff4fa6083d08b42fb2a7ba1a33d007807079de445`。本票仅给 gateway 增加现有 workspace `protocol-anthropic` 依赖边，没有更改外部依赖版本。
- 固定 debug gateway：`output/issue-71/bin/gateway-final-tested`；SHA-256 `cbdca45f6a33bbbb438ec54092553aea381d962669765f3b0b88f0a360573fe9`。
- `output/issue-71/source-03.json` 固定7219个受控文件、工具链、环境、工单正文/原生阻塞输入、固定 Go 文件与 HTTP 场景；manifest SHA-256 `2e6a9094ed12f92ca91decb368580afbebdd4111e2dbf4a64274ee2b8b005c65`，受控文件集合 digest `ee53c78ceea8f56020ff23092d719bce8ef913b5cf3b9023f03836095ed646f7`。
- 构建/测试使用独立 target/TMPDIR、cargo jobs2、测试线程4、动态 loopback 端口；业务数据库和受控状态在本票 output，后台 Provider workers 未启动。未读取真实凭据。
- 原 source-01/02、原二进制、原失败日志保留。最新文档是受测代码后的 docs-only 后继，不把其 HEAD 冒充 Full 启动时的源码。

[机器汇总](issue-71-request-rules-upstream-headers-20261025-evidence/summary.json)包含最终 binary/input/log/test-binary 清单 hash；
[Full回执](issue-71-request-rules-upstream-headers-20261025-evidence/full.md)和[审查记录](issue-71-request-rules-upstream-headers-20261025-evidence/review.json)分别记录动态与静态证据。

## 实现与 CPA 对照

固定参考 CPA v8.0.40 / `d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683`：
`helps/payload_helpers.go::ApplyPayloadConfigWithTrackedPathsForExecutor`、
`openai_compat_executor.go::executeWithCredentials`、`util/header_helpers.go::ApplyCustomHeadersFromAttrs`，
以及 `claude_executor_tokens.go`。实际 Go 输入文件 hash 位于 source manifest；保留既有 MIT 署名。

| 范围 | 本票确定的行为 |
|---|---|
| 渠道/目标协议 | Endpoint 及其 Upstream 限定渠道与目标协议；支持普通 Chat/Responses/Messages compatible adapter 及其现有 OAuth 渠道。专用 Grok Build/Console/Web/Official、Kiro 保留固定 profile，非空策略明确拒绝 |
| 模型/下游协议 | 匹配已授权、已解析的公共模型精确名称，或单独 `*`；可选三个下游协议。Alias/prefix/suffix/upstream model 不创建额外身份或权限 |
| 顺序 | 原入口认证/历史与控制校验 → #69 源提取、名称/模型规范化及协议转换 → Provider body → 全部 default（first wins）→ 全部 override（last wins）→ 全部 filter → 最终目标校验 → 安全 header 组合与传输 |
| Provider/控制优先 | stream/store/model/auth/framing、thinking/tier/cache/tool/metadata/owner/history 均不在可改字段中；Provider 必需字段保持，规则不改 #69 决胜与准入。降低 Messages 输出上限不能靠改写 thinking 来掩盖预算冲突 |
| 白名单 | 仅公开契约列出的目标协议 sampling/output root 选项；没有 JSON path、raw、脚本、条件表达式或新配置服务 |
| default 差异 | 按已转换目标 body 的字段存在性补缺，null 为 present，包含 translator 合成字段；没有冒充 CPA originalTranslated/Claude max-token 特例 |
| 冲突 | 同条规则重叠操作、非法字段/值/目标在保存时拒绝；跨规则按模型与协议分区检查最终双输出上限，最终 filter 可合法消解；客户端值才造成的碰撞在实际发送前拒绝 |
| header 秘密 | 名称映射到同版本、同 Upstream、active且无 serving binding 的 `upstream_header` Credential ID；复用 Secret Store/AAD与写入即加密。准备时解密校验，运行态采用 redacted/zeroizing PlaintextSecret |
| header 边界 | 名称/值/CRLF/control/ASCII/单项及总大小有界；账号、内部、Origin/协商/传输必需名称受保护，追加项不能覆盖已有头；没有客户端 header 或 session 变量插值 |
| count | 全部规则字段为生成选项，不修改输入计数内容；第一方 native count 加 Endpoint headers，本地估算不发上游；不产生推理 Attempt/Usage/ledger/quota |

完整白名单、数值/大小限制、保存/发布/回滚样例见
[公开支持说明](../support/request-rules-upstream-headers.md)、[ADR-0134](../adr/ADR-0134-versioned-request-rules-upstream-headers.md)、
[BC-MGMT-022](../contracts/BC-MGMT-022-request-rules-upstream-headers.md)。
有意 override/filter 是持久化、可审计的配置行为和兼容转换，不承诺无损；LosslessBridge 拒绝 body rules。

## 逐项验收

| 工单要求/继承边界 | 本次结果及证据 |
|---|---|
| 模型/协议/渠道与三阶段顺序 | 本地 PASS；三目标 × 三来源 × JSON/SSE 共18次配置矩阵；额外 exact-model/protocol 作用域、suffix 控制和静态重叠分区回归 |
| 版本化保存、校验、发布、回滚与审计 | 本地 PASS；真实管理 socket、生产生命周期/RuntimeFactory、公共 socket 到独立 receiver；fork/reseal、stale publish、并发相同 revision 仅一个写入获准 |
| 保护名称、权限、内容和历史 | 本地 PASS；保护字段拒绝、跨 Key/model 拒绝；两次工具调用/结果及后续追问共30个成功请求；owned stored response 合法续接、foreign GET/续接拒绝且零新增发送 |
| 自定义 headers 与保密 | 本地 PASS；实际 receiver 同时核验 custom header、账号授权与 Content-Type；非法名称/值/换行/大小/总量、foreign owner/AAD/account-kind/binding 负例；普通配置、响应、审计及 Debug 无明文 |
| 冲突、非法路径/值、不适用目标 | 本地 PASS；HTTP400和稳定安全 code、core 分区测试、专用 adapter/Lossless 拒绝；client-dependent limit/budget 负例零发送 |
| 实际 JSON/SSE 与回滚 receiver | 最终冻结源码的专项：Responses22、Chat20、Messages19，共61个推理 receives；原生 count1，本地 count0。数字含 baseline、矩阵、工具、名称、存储、未命中模型及回滚，不把所有61次说成同一个场景 |
| count 不制造推理用量 | 本地 PASS；生产 count composition 后查询 event store、BillingWorker/ledger 和 quota history 均无推理记录；估算 provenance 不改 |
| OpenAPI/契约/样例 | 本地 PASS；三个新 schemas、Endpoint input/output 字段、完整本地引用解析、公开限制与使用样例。没有新前端 |
| #69/#73/#75/schema40 | 本地受影响回归 PASS；Full保留名称/控制/权限、精确定价与旧账本/冻结查询测试；新 schema40→41 fixture 保持原密文与凭据 revision；有规则配置 fork及v2备份读回使用原AAD/keys成功 |

推理61次与count1次均使用合成凭据、独立 loopback receiver。它们是本地受控 HTTP 证据，
不是 stock process、固定 Build 服务、真实 Provider 或原版客户端的验收。

## 门禁与专项

| 检查 | 结果 |
|---|---|
| 项目 Full03 | **44/44 PASS，exit0**；2026-10-25 18:11:56 Asia/Shanghai完成 |
| Rust workspace/all-features | **1733 passed /0 failed /23 ignored**，136个结果集合 |
| 最终固定源码 HTTP专项 | **3/3 PASS**；61 inference receives +1 native count；本地估算零发送 |
| 显式 encrypted_backup --include-ignored | **7/7 PASS**；含128MiB上限拒绝；普通Full里的ignored未转算通过 |
| schema40→41 +原密文 | 新增迁移测试在Full通过；完整唯一1..41前缀，原密文/revision与空规则保持 |
| 管理 OpenAPI | 18项契约测试通过，本地引用全部可解析；414项行为契约引用解析通过 |
| 供应链 | cargo-deny四类策略PASS；cargo-audit扫描378依赖无漏洞；原重复版本warning保留 |
| 源码固定 | 测试前后7219受控文件hash一致；stock/test binaries及logs hash分别记录 |

Full 的 ignored 原因与原标记保留。显式备份专项单独列出，不把 ignored 计为普通 Full 的通过。
本票未重跑旧 schema39/40 的94MiB级冻结容量分布或198项 wave2联合进程；没有把旧回执改成新 schema41 PASS。
当前 v2 配置备份/恢复、原有大小边界、加密/错钥/完整性测试分别按本次实际结果记录。

## Standards

初次独立45文件规范轴发现1项：OpenAPI schema 放错 components 分组。`efe0fb1a` 修复，
后继独立六文件规范复核无待修问题或 smell。Full02 后的一行 fixture SQL 修复由主代理核对明确旧10列与保留的全部断言，专项通过。

## Spec

初次独立规格轴发现2项：同一 OpenAPI 问题及可静态判定的跨规则输出上限冲突。
均在 `efe0fb1a` 修复并有回归。后继 Spec 探子核实六文件修复，未见新增问题，但最后一次精确调用点检索因不存在的 shell glob 中断，保持 PARTIAL 记录。
主代理按明确文件/行补查 recursive local-ref helper、create/update endpoint、compiler、runtime preparation/application，完成余项，无待修发现。
两轴分别保留，静态审查没有代替动态测试。Standards初审1/最终0待修；Spec初审2/最终0待修。

## 共享接口、迁移与集成注意

新增 `EndpointConfiguration.request_policy`、Endpoint API字段和三个 OpenAPI schemas；现有 revision/audit/fork/publish/rollback 链未替换。
新增 Credential kind `upstream_header` 和受保护 `InvalidRequestPolicy` 错误；新增 transport 的安全追加方法、普通 send path 的 Candidate 参数及 runtime preparation。
依赖仅新增现有 Anthropic codec 的 composition edge。没有实施 #70 选择策略或 Build affinity。

migration0041 仅为 `upstream_endpoints.request_policy_json` 添加默认空列，沿用完整1..40前缀。
ADR0134与并行 #70 的0133不同；批次协调文件为项目容器
`maintenance/2026-10-25-wave3-development-prep/COORDINATION.md`。

**后续只读检查发现 #70 也独立用了 migration0041。两条分支的 schema41 不是同一种 schema。**
共享集成必须先统一迁移身份/顺序，保留两票功能；不得直接交换这两种库或备份，或只看数字41就宣称兼容。
需采用共同 schema40 artifact 或经审查的集成转换方案；本票没有导入/改写 #70 工作区，也没有宣称集成通过。
正常功能回滚使用既有配置版本；schema41→40 DROP COLUMN 会丢策略配置，不是无损功能回滚。
外部 Master Key 与独立 admin-auth 存储仍不在备份内，原 reader/model_thinking/schema39 资格限制保持。

## 历史失败与保留

- 初期 check-01/02、HTTP01/02/04/05及Clippy01–05的编译/测试/静态失败保留在 `output/issue-71/logs/`，修正后使用新日志名。这些早期WIP没有逐次固定完整源码manifest，其精确受测源码身份保持UNKNOWN，不回填为后来的commit。
- Full01 在独立审查发现问题后停止，**INTERRUPTED/143**；受测 `436f9e6d`，manifest与固定binary保留。
- Full02 在 `efe0fb1a` 上 **FAIL/101**：`native_deletion_preview_is_concrete_revision_guarded_and_preserves_shared_graph` 的旧位置式 INSERT 给新增11列的表提供10个值。
  `2dc8c87c` 只显式列出旧10个目标列，使新列使用空默认；原删除预览、并发review/revision、共享图保护断言没有删减，原失败和57项inventory套件的上下文保留。
- crates SQL 探子的主要检索完成并定位唯一问题，末尾补充 glob 检索失败保留为partial工具记录；另一探子 apps/scripts/tests 范围完成，未发现同类 endpoint SQL。
- 本票主要HTTP场景中仍存在的私有合成状态、数据库、未发布产物及原始日志保留在 ignored output；既有临时测试夹具按原生命周期清理，不声称每个旧失败库都仍存在。报告不复制原始 headers、tokens、密钥或数据库内容。

## 剩余资格

支持范围限三个普通兼容 adapter 和本机 macOS debug/合成输入。本票不增加专用 Grok/Kiro payload 配置能力，也不取消其原有产品承诺。
真实 Provider/费用、真实官方 count、未修改 Codex/Claude/Pi、固定 Build、远端CI、release/OCI/Linux与部署均 NOT_RUN；
`cost_in_usd_ticks` 仍 NOT_ADMITTED，#61–#67及首版READY条件未解除。
GitHub #71保持本次读取的 OPEN/无原生blocker；本地交付不等于远端关闭或整个CPA v8全量等价。
