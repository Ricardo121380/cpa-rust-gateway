# CPA-Rust #62 C1–C3 原生 namespace/function 独立切片

本报告只验收原生 namespaced function 身份全链路的软件实现和本地受控装配。本切片已达到 **PASS_BOUNDED**。固定 Build profile 1.0.50 尚无 namespace 字段资格，生产启用保持 **BUILD_ACTIVATION_BLOCKED**；受控成功不代表原样 Codex 兼容、真实 Provider 可用或 #62 整票完成。

## 基线、隔离与来源

- 最新交付源码实际基线：`bbe3a53f9163fc55f6eb699a12983fee0c350210`，原分支 `feature/65-evidence-process-recovery`。参考 SHA 仅定位，没有回退。
- 原工作树：`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar65-evidence-process-recovery-20261020`。
- 本轮工作树：`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar62-native-namespace-20261020`，分支 `feature/62-native-namespace`。
- 软件实现提交：`2a25f325dbfc69a4a5e869398fb93b430d258430`；审查发现的 C3 资格缺口修复提交：`5e62cb99291a2444282b4f25f3a49fb404a5287e`。
- 原树 18 项既有未提交文档/规则逐项带入，来源、原始状态、哈希、mode 与验证树实际应用哈希单列绑定。包括仍为 REVIEW_DRAFT 的字段方案；它没有因此被整体批准。三个共改文档采用原始 overlay → 本轮增量的独立暂存，其既有改动没有混入提交。
- 没有 reset、clean、stash、广泛暂存；原树 HEAD、分支、暂存区、dirty 状态、diff 及这 18 项字节/mode 单独复核。

## 完成判定与最终验证

| 范围 | 状态 | 实际证据与限制 |
| --- | --- | --- |
| C1–C3 本地原生声明→控制→Build→JSON/SSE→结果→历史→两次相关工具续轮与问答 | **PASS_BOUNDED** | JSON、1/7/31 字节 SSE 四种完整受控路径；每种五次独立接收与同 Attempt/Usage/ledger 关联；定向测试 7 passed |
| 正常生产 namespace / 新增结果身份成员准入 | **BUILD_ACTIVATION_BLOCKED** | 固定 Build 1.0.50 字段资格缺失；未资格请求预拒绝，独立接收零；仅结果 name 也不能绕过 |
| Build parallel=true | **BLOCKED** | 实际 Build capability ledger 未取得 ParallelTools；没有因 namespace 切片追加能力 |
| 原样 Codex / #62 整票 | **FAIL / BLOCKED** | 未改请求 HTTP400，CLI exit1、六项成功门禁全 false；#61 等依赖与 C4/C5/C6 未解决 |
| 最终不可变源码 Full | **PASS（携带明确 overlay）** | 45/45 PASS，Rust **1587 passed / 0 failed / 16 ignored**，555 秒；ignored 不视为执行成功 |
| Standards | **PASS** | 首次完整 30 路径审查 PASS；9 文件资格修复独立 Standards 复核 PASS，无未解决发现 |
| Spec | **PASS（本切片）** | 完整切片审查发现 P1，动态 RED 确认；修复后独立 Spec 复核闭合。未宣告 #62 原 AC 全部满足 |
| 真实 Provider / 真实账号 / 收费 / 远端 CI / 发布部署 | **NOT_RUN** | 本轮未授权，也未以受控装配替代这些证据 |

最终 Full 绑定源码 `5e62cb99291a2444282b4f25f3a49fb404a5287e`、tree `a2853d180df9825ab3fe148b44f55a55c09fca44`，在独立验证树 `/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/validation/cpar62-namespace-20261020-5e62cb99291a` 使用该树自己的 manifest/lock 和独立 target 执行。7,019 项输入前后完全相同，记录集合 SHA-256 为 `ee02b9be75df6c07d81dcf409c564e735139676f5515c3896e3944bcde9ee083`。这是带 18 项已绑定既有文档/规则 overlay 的 Full；不称为无 overlay 的纯产品树验证。后继交付提交只添加本报告与证据，不更改软件输入，文档增量另做静态检查。

**保留的首次 Full 失败：** 同一候选首次 Full 的 gateway 结果为 260 passed / 1 failed / 4 ignored。既有 `public_build_upstream_eof_idle_and_total_timeout_fail_once_without_resend` 预期已提交 SSE 头后的 HTTP200，实际 HTTP503。原因 **UNKNOWN**。同一二进制、原断言/原超时不变的定向重跑通过（1 passed，7.58 秒）；未更改源码再次执行完整 Full 后全通过。本报告保留首次失败，不推定为生产根因或已经解释的环境问题。原始两轮日志、定向重跑和源码绑定均在证据包内。

Full 覆盖受影响 flat function、Pi 所依赖的工具/失败/取消路径、Responses summary/Usage、Messages summary 及现有存储边界。既有 ignored 实际客户端/#65 opt-in process 矩阵没有被默认 Full 执行，本轮只单列运行指定的既有 #62 客户端隔离驱动。

## 已实现的闭合行为

1. **声明与身份。** 纯 Responses codec 的 `ResponsesToolSet` 对已有 `openai.responses.tools` 扩展提供闭合验证视图。只要声明含 namespace，保存整个原生有序数组，保留组名、description、顺序、child 名称/schema/description/strict 的 presence，拒绝与 typed flat 表示竞争。身份为 `(namespace?, name)`，call_id 独立关联；不拼接、不扁平化。旧纯 flat 工具仍走原 typed 表示与序列化。custom、deferred、嵌套、未知成员、同组重复和矛盾继续拒绝。
2. **控制与历史。** auto/none/required、flat forced selector、parallel/serial 与完整声明集合共同校验。跨组同名 child 可以区分。没有依据的 namespaced forced selector 发送前拒绝。串行响应第二次工具调用失败；parallel=true 仍要求实际 ParallelTools 能力。显式历史按顺序和 call_id 核对完整身份与完整对象参数，不按裸 name 匹配。
3. **返回与完成屏障。** Build JSON 和 SSE 的 item added、参数 delta/done、item done、terminal 保持 item/output index、namespace/name/call_id 及参数一致。标准参数事件没有自造 namespace 字段。受控 namespace 路径在 terminal 身份/顺序/参数验证前不输出可执行 ToolCallEnd 或 function item 完成；Usage 投影先于这些完成事件，防止未准入 cost 导致失败前暴露虚假工具完成。失败、EOF、取消或 terminal 冲突不冲刷暂存完成事件，不重发。
4. **工具结果。** call_id 关联，独立保留可选 name 和 namespace 的缺失、null、实际值；实际字符串必须确认对应调用，null 只表示未指定，不补造。仅有 result.name、整份请求没有 namespace，也属于新增 C3 字段资格范围，不能绕过生产门槛。
5. **受控资格。** 公共 listener → 鉴权 → 实际 compiler/factory → leased Build adapter → 固定 outbound URL 的既有传输边界 → 拥有的独立 loopback receiver → writer。测试装配必须显式设置私有 cfg(test) 字段并提供受控 transport；只有 transport 注入不能赋权。依赖 crate 的受控 API 通过明确的 dev-dependency feature 提供，未假设 gateway cfg(test) 向依赖传播。普通装配资格恒 false；实际 adapter、Responses 源/目标及 Canonical/CanonicalBridge 必须共同成立。没有客户端字段、头、User-Agent、用户配置、宽泛能力位或任意 URL override 入口。
6. **依赖和持久化。** 复用纯协议视图，Router 对 codec 的窄依赖保持无环；core 不依赖 protocol。未改 Canonical 持久结构、SQL schema、payload v1、AAD、key/TTL/reader。已有加密 envelope 的 namespaced 声明、事件和结果 presence 经 reopen、key rotation、owner 隔离和幂等 replay 验证。该单元边界不冒充公共 stored-response 完整链或 #65 恢复矩阵。

后继决策为 [ADR-0115](../adr/ADR-0115-native-namespaced-function-identity.md)，对应更新 BC-CORE-002/003、BC-ROUTER-004、BC-PROVIDER-003、[BC-PROTOCOL-009](../contracts/BC-PROTOCOL-009-codex-responses-controls.md)。旧 Accepted 决策未被覆盖重写。

## 受控证据与 RED 保留

- 基线公共 namespace 请求仍保留 expected HTTP400、receiver/Attempt=0 的负例；测试退出 0 只表示拒绝断言成立。
- 正例 JSON 和 1/7/31 字节碎片 SSE 各完成五次请求：`read_side.lookup` → 结果 41 → 回答 → `write_side.lookup(previous=41)` → 结果 42 → 回答及后续问答。每种模式独立接收/Attempt/Usage/ledger 均为 5，租约最终为 0；声明完整、参数、call_id、summary、20/7 已知 Usage、完整历史和顺序逐次核对。
- 受控主链使用 `parallel_tool_calls:false`。这是按真实 Build 能力账本接受串行；没有新增或伪造 ParallelTools 资格。parallel=true 的正常和受控 Build 装配继续预拒绝；纯 Router 的有资格 parallel 正例另验完整身份语义。
- 预发送负例覆盖重复组/child/flat、custom/deferred/嵌套、未知成员、strict:null、非法 selector、结果/调用身份及 call_id 不匹配、重复参数名和未资格请求。独立 receiver 的 accept/request 数和 Attempt/lease 均为 0。
- SSE 20 类错误分别按 1/7/31 字节验证，包括 added 缺身份、delta/done 改写、terminal 丢失/改写身份、额外 cost、EOF、参数未完成/数组/重复键、重复 done、failed-after-done 和 serial 第二调用。每例一次接收/Attempt、一个失败、零工具完成、无 retry。JSON 的身份/参数错误另外失败一次。
- auto/none/required 的允许和矛盾、flat forced selector 不误选同名 child、结果 name/namespace 各自缺失/null/value 的九种组合都独立验证。
- 审查 P1 的动态 RED：无任何 namespace 的 flat 历史仅加 result.name，普通装配修复前返回 200 而应 400。修复后 JSON/SSE × null/string 均预拒绝/零接收；受控装配精确保留完整原历史。该 RED 和 GREEN 分别保留，未靠改断言放行。
- 开发中保留首次 parallel 能力拒绝、历史 scalar oracle 修正、Usage 投影前虚假完成 RED 以及未完成的公共 stored fixture 尝试；最终结论只使用最终候选验证。公共 stored fixture 的 RouteAccess 失败不算存储成功；采用的存储证据仅为上述已有 envelope 单元边界。

## 原样客户端独立观察

既有隔离驱动和客户端程序没有修改；命令保留 `--require-codex-success`。第一次驱动因传入 `/private/tmp` 而未通过实际 OS 临时目录约束，CLI 尚未启动；按 `tempfile.gettempdir()` 的实际路径重新装配，失败记录保留。

- 本轮实际版本：Codex **0.161.0**、PATH Pi **0.84.3**、Node **v26.8.2**。Codex 原生可执行文件 SHA-256 为 `12ac11d2c7eee27cfae34393986d7b7c9ed0dea537cb749831cdd7033893e6de`，按实际 launcher 的平台包解析/回退确定，与安装前 manifest 相符；完整安装字节/mode/link 的前后比较未变。Pi 实际 PATH 入口、声明 bin 版本探测和加载模块图分别冻结；没有执行 only-my-pi。
- 安装树的 OpenAI SDK 为 6.40.0；没有观察到独立的外部 `openai/responses` ESM 模块加载，不用该安装版本推定实际嵌入 SDK 版本。实际 Pi 入口和已观察模块的哈希保留。
- 每个场景真实执行 OS 探针：非 loopback IPv4/IPv6 连接、真实用户目录读和临时目录外写均 EPERM；使用独立 HOME/配置/工具目录，无真实凭据，不以 `PI_OFFLINE=1` 代替隔离证明。

| 场景 | 实际结果 | 公共响应 / 独立接收与记账 | 边界 |
| --- | --- | --- | --- |
| Codex standard | **FAIL / BLOCKED**，CLI exit 1；六项正向门禁全部 false；外层驱动 exit 1 | 实际请求 HTTP400；独立接收/Attempt/Usage/ledger 均 0 | 原 expected-400 准备测试 exit 0 只是负例；正向场景驱动未完成，#62 仍未完成 |
| Pi standard | **PASS_BOUNDED**，三次 turn outcome 均 stop；两次相关工具、实际 result.txt=42 和后续问答 | 五个增量 HTTP200，独立接收/Attempt/Usage/ledger 各 5；另有一个已标记拒绝探针 | 仅原串行显式历史路径；不授予生产 namespace、ParallelTools 或真实 Grok 资格 |
| Pi failure | 网关失败链回归通过；客户端用量呈现保留既有限制 | 一次接收/Attempt，单个 response.failed，无 completed；已接收 Usage/ledger 各 1 | Pi 显示的失败用量仍为全零，不代表上游零调用/零成本；计价 UNKNOWN |
| Pi cancel | **PASS_BOUNDED**；实际 delta 后 lease=1 → abort → 同 PID 存活 → 独立上游关闭 → lease=0，均在 cleanup/server stop 前 | 一次接收/Attempt，无 completed；13 ms 是本次释放观测上界，fast probe 与 eventual 均通过 | 未接收终态 Usage，记录 0 行仅是缺失证据，成本 UNKNOWN；不推定生产 SLA |

本次 Codex 完整原请求仍携带 `namespace multi_agent_v1`、`web_search`、`client_metadata`、summary-only reasoning、parallel=true 和 tool_choice=auto。没有删除字段、关闭工具、降级客户端或改客户端代码。它不能由合成 namespace 正例代填。本轮观察也不能证明未改请求已经实际走过每个后续字段拒绝点。

## 验证 overlay 逐项来源

下表的原始字节均来自原交付树的既有未提交内容，不把它们冒充已提交源码。完整 SHA-256、原状态、mode、验证树应用哈希和候选 blob 见 overlay-binding.json；三个共改路径的应用值还包含本轮已提交增量。

| 路径 | 原始状态 | mode | 原始 SHA-256（前16位） |
| --- | --- | --- | --- |
| `AGENTS.md` | `tracked_modified` | `0o644` | `810c5eac53766af3` |
| `CONTEXT.md` | `untracked` | `0o644` | `eaa545ede563d661` |
| `README.md` | `tracked_modified` | `0o644` | `376de5be81e5b8ac` |
| `README.zh-CN.md` | `tracked_modified` | `0o644` | `d5ae62085c88f420` |
| `docs/06-development-plan.md` | `tracked_modified` | `0o644` | `322ac4c20b44eedf` |
| `docs/adr/ADR-0100-exact-protocol-semantics-and-usage-evidence.md` | `tracked_modified` | `0o644` | `ad35d32a70725f9d` |
| `docs/adr/ADR-0101-grok-native-item-metadata-fidelity.md` | `tracked_modified` | `0o644` | `a426616e9a4cbb87` |
| `docs/adr/ADR-0106-native-fidelity-profile.md` | `tracked_modified` | `0o644` | `8e770df75f08f2ce` |
| `docs/adr/ADR-0107-standard-response-metadata-separation.md` | `untracked` | `0o644` | `dad1f8391de1a6d8` |
| `docs/adr/ADR-0108-build-summary-standard-messages.md` | `untracked` | `0o644` | `a8088457f16a6f60` |
| `docs/adr/README.md` | `tracked_modified` | `0o644` | `1da966c9a0826a38` |
| `docs/agents/domain.md` | `untracked` | `0o644` | `143fbbb108382fe7` |
| `docs/agents/issue-tracker.md` | `untracked` | `0o644` | `c95aff9dd13f35be` |
| `docs/agents/triage-labels.md` | `untracked` | `0o644` | `ef4786ebc27973fe` |
| `docs/contracts/BC-PROTOCOL-009-codex-responses-controls.md` | `tracked_modified` | `0o644` | `4a4c97b377e5fe21` |
| `docs/engineering-roadmap.md` | `untracked` | `0o644` | `0babeedb83b7c213` |
| `docs/reports/cpar-62-64-client-field-plan-20261020.md` | `untracked` | `0o644` | `26cb7b1a5af0ee25` |
| `docs/traceability.md` | `tracked_modified` | `0o644` | `8ef39ca0dc80a221` |

## 剩余边界与下一项最小任务

固定 Build 资格最小缺口仍是目标服务方固定日期/版本、适用于 proxy/profile 1.0.50 的声明或可核验 source/release 证明，须覆盖 function-only 分组（含 strict:false）、auto/parallel 控制、JSON/SSE 完整身份、参数、结果与显式历史。受控 receiver 接收成功不提供这一资格。

C4 client_metadata 的本地消费 D1 尚未批准，继续预拒绝；C5 web_search（含 external_web_access:false）没有完整 hosted-tool/来源引用/Usage 契约；C6 非 assistant input message id 仍待标准历史与固定目标资格；#61 cost_in_usd_ticks 继续 NOT_ADMITTED，成功元数据留存准入未取得。Claude thinking/effort/cache 和 D2 配置也未实施。

下一项最小任务是核对新增的、适用于固定 Build profile 的 C1–C3 资格材料；未获得新材料时保持生产阻塞。软件侧另开 C6 非 assistant input id 的窄切片做准确历史语义和目标资格判断，不连带决定 D1、search 或 cost；原样 Codex 正向驱动须等 C1–C6 条件齐备后单独修改现有具体场景，不能只使用 --require-codex-success 门禁代替任务驱动。

本轮没有真实 Provider、真实账号或收费请求，没有 push、GitHub 工单/依赖写入、合并、发布或部署；没有新增网关多 agent 执行器，没有重做 #65 或推进 #66/#67。

## 交付证据索引

- [源码、manifest/lock 和两个测试二进制绑定](assets/cpar62-native-namespace-20261020/source-binding.json)
- [18 项 overlay 的原始来源与实际应用值](assets/cpar62-native-namespace-20261020/overlay-binding.json)
- [Full 两次结果及限制](assets/cpar62-native-namespace-20261020/full-result.json)、[最终 45 步明细](assets/cpar62-native-namespace-20261020/full-check.md)
- [独立双轴审查及 P1 闭合](assets/cpar62-native-namespace-20261020/reviews.json)
- [最终受控收据](assets/cpar62-native-namespace-20261020/controlled-receipts.json)：四个五请求正例，78 个零接收预拒绝（含仅 name 的四个负例），60 个 SSE 无虚假完成负例；仅 name 的另四个受控正例精确保留历史。
- [原样客户端、身份与隔离观察](assets/cpar62-native-namespace-20261020/client-observation.json)
- [原交付树保全收据](assets/cpar62-native-namespace-20261020/preservation.json)
- [受控原始证据包](assets/cpar62-native-namespace-20261020/verified-evidence.tar.gz)、[逐成员哈希、读回与秘密扫描](assets/cpar62-native-namespace-20261020/evidence-manifest.json)

证据包只收入明确 allowlist 的合成请求/响应、客户端隔离收据、测试/检查日志与绑定清单；不收入临时凭据配置、账号目录或数据库。每个成员读回一致，嵌套 gzip 解包后继续扫描。源码清单的一个既有 task 文件名及 Cargo 的两个 futures_task 产物名命中 token 前缀子串，被验证为文件路径误报；只有这些精确路径在指定清单/编译元数据内具有已记录例外，其余内容照常扫描，原始字节未改写。
