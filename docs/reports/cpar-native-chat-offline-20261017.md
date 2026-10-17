# CPAR-38 Grok Build 原生响应离线诊断与严格 Usage 修复

本批基线 `ec534918647a483344dfd5dbb4abc2a36fb8c5d8`。**2026-10-15 样本的首个拒绝根因已证实：reasoning summary 进入 Chat/Messages 投影时没有已批准的等价表示。该拒绝符合 ADR-0101/0105，不是可无损表达却被误拒。** 完整离线检查另外复现了两个实现缺口：Build 丢弃未映射 Usage/不核对 total，以及投影拒绝 Usage 本身时失败收口失去该事件。已做最小修复；丰富 summary 仍严格拒绝，三协议目标仍未完成。

真实 Provider 调用 **0**，新 Pin/真实 Chat/远端验收实例/部署均 **0**。原始真实 Chat FAIL、旧失败及已停止预算保留；本批本地测试通过不升级真实渠道结论。批次约束见 [batch-contract](assets/cpar-native-chat-offline-20261017/batch-contract.md)。

## 身份与私有边界

原始捕获通过一次只读 SSH 从永久停止的 `chat-json-serving-20261015` 取回本机保护目录。没有启动/停止服务、管理 HTTP、凭据读取或新 Provider 请求；旧 unit 仍 `not-found/MainPID=0`，budget stopped。采集脚本只读取明确文件及只读 SQL，将原始字节直接传给本地程序；原文没有进入工具文本输出、Git、GitHub 或审查输入。

| 对象 | 本批实际核验 |
| --- | --- |
| 原生传输 | HTTP200 / Json / Gzip；完整压缩体673字节 |
| 压缩体 SHA256 | `7b5a3c2766766397af638665aaa7160f351780232a2c06f11d8f784ebbfdaa96` |
| 解压 | 1393字节；SHA256 `1105cd2ae1bebc25bc75794057cee7851e9f508980682650effeaa34b0a58ab1` |
| 原请求 | POST `/v1/chat/completions`；grok-4.5；stream=false；max_completion_tokens=4096；原字节未改 |
| 请求 SHA256 | `245c50f95d487dbfa3333f0e33eb4978ea2cedd2c3d9bd1f1010aafbaaab6062` |
| Request | `p1-request-71cc49273ebb7382fa61cd5f984e8c51-0` |
| Attempt | `attempt:45:p1-request-71cc49273ebb7382fa61cd5f984e8c51-0:1` |
| 原候选产品 source | `bf822b14a62c21cf582a7539e3635f69bd8b7677` |
| 原 binary SHA256 | `cdadcb2cd679fe02e291f7a02fd6714aa92df816a91a5624858112ad19818b1e` |
| 原 manifest SHA256 | `655c037df68c5657b250105bbb07bed665210a7f6b20ea8d8b88aab488157a56` |
| 原 metadata SHA256 | `d5e87f208ec6ab4265a272d4be5d331bbb0865e30abfa62fa6556671d96cb78c` |

取回文件逐一与旧退出清单的长度/摘要核对；本机旧binary/manifest/metadata重新哈希一致。selection、body-receipt、start、原chat-summary、Pin execution及SQLite sidecar/Attempt payload的摘要与身份一致。实际lease revision153由Pin与sidecar相互核对，不从native152或pool推算。历史Request failed、唯一Attempt succeeded、Usage1、ledger1/source-event精确对应，cost仍null。详见 [acquire](assets/cpar-native-chat-offline-20261017/acquire.json)、[identity-and-shape](assets/cpar-native-chat-offline-20261017/identity-and-shape.json)；[structure](assets/cpar-native-chat-offline-20261017/structure.json)只含白名单字段/类型/计数/hash。

## 首个拒绝及后续完整链

生产路径为 `GrokBuildInferenceAdapter::execute` → `GrokBuildResponsesDecoder::decode_owned_with_encoding` → 有界gzip解压/严格JSON → `handle_response_created`、逐item added/done、terminal → `BufferedEventSource` → runtime `ProjectedEventSource` → `ProtocolResponseProjector::project_event` → 公共Chat handler/codec。

同一673字节样本实际产生11个合法Canonical事件。零基序号0 ResponseStart，1 MessageStart，2 reasoning OutputItemStart，**3 ReasoningDelta，4 reasoning OutputItemEnd**，5 message OutputItemStart，6 TextDelta，7 message OutputItemEnd，8最终UsageDelta，9 MessageEnd，10 ResponseEnd/end_turn。

首个错误是序号3：原生路径 `$.output[0].summary[0].text`，Canonical扩展 `openai.responses.output_part` 的 `field=summary/index=0`，关联前一reasoning item。`crates/gateway-router/src/response_transform.rs::project_native_identity` 的 `value["field"] != "content"` 检查返回 `UnknownExtensions`；runtime保持分类，公开错误为 `UpstreamProtocolError`、固定response metadata消息、HTTP502，并附已准入Request ID。不是gzip失败、未知item字段、上游4xx/5xx、凭据失效或请求输出上限拼写错误。

序号2的开始元数据将summary初始化为空数组，所以先通过；完整summary由delta及序号4结束元数据确认。合成空文本summary仍在结束元数据以 `ReasoningUnsupported` 拒绝，证明只绕过第一个delta检查仍无法合规完成。没有请求thinking也不得删除该reasoning。原body缺失，不能把summary的内容或reasoning_tokens计数当作完整body。

此外，同一完整样本有四个标准计数之外的Usage根字段：两个显式零来源/服务端工具计数、一个非零 `cost_in_usd_ticks`、一个未解释object（公开仅key SHA256 `3c425705071574051d6bc92d91df15a0787041a964e8caf9d1132f804a7c6fa0`，含两个非零integer子字段）。它们此前在 `parse_usage` 被丢弃。旧实现因而对完整样本给出Responses投影PASS，**该结果不是完整原生保真证据**。修复后同一样本Responses在序号8 UsageDelta明确拒绝；Chat/Messages仍在序号3拒绝。原生价格状字段的Build含义、单位、结算归属没有独立证明，真实费用仍UNKNOWN，不回填账本。

## 跨协议表达性与A/B/C判定

| 保存的信息 | Responses | Chat / Messages | 判定 |
| --- | --- | --- | --- |
| reasoning summary一项，content缺失 | 原生item/part可表达并保留hierarchy/顺序/身份 | 当前reasoning_content/thinking仅有plain body，不能承载summary语义 | B：严格拒绝正确，契约阻塞 |
| message一个output_text，annotations=[]，logprobs=[] | output逐项精确保留，数组/空值不改 | 单独plain text可表达；不能抵消summary拒绝 | 已支持组成部分，不等于整个样本PASS |
| item id/status/role、part索引、终态顺序 | Canonical及native codec保留、相互确认 | 仅现有plain reasoning/message/tool桥接可消费其身份封套；丰富结构仍拒绝 | 现有契约不变 |
| input228/output159/reasoning158/cached128；缺失cache_read/creation=null | 标准Usage及cpar_usage保留 | 同样保留六个源计数与evidence | 已有映射；丢弃扩展/失去已知快照是独立实现缺口 |
| total_tokens=387 | 完整计数可由228+159确证；不制造缺失计数 | 同样精确算术；非法/矛盾total拒绝 | 实际样本无total矛盾；合成负例证实旧缺口 |
| 四个额外Usage字段，包括显式0及未解释object | 当前公共Usage/账本无已批准完整映射，保留raw extension后拒绝 | 同样无映射，不能删除来取得成功 | B：目标表示不足；C：未解释字段含义UNKNOWN |
| 原生模型/时间、控制回显、metadata、service/store等根封套 | 当前codec的public model/created_at由路由边界提供；不是原生root整包回显 | 无完整原生root封套对应项 | 不宣称整包wire无损；未获批准的字段映射保持未解决 |

原生model字符串与原请求public model不相等，实际值未公开。现有 `OpenAiResponseMetadata` 明确由路由边界提供公开模型和创建时间，不能从该不等式推断错账号；也不能声称已经证明原生别名的等价关系。旧正式 `verify_success_fidelity` 要求native/public均字面等于grok-4.5，故即便另行解决summary，也仍有这一验收前提需要先核对。没有修改该oracle或原请求。

[xAI公开Responses参考](https://docs.x.ai/developers/rest-api-reference/inference/responses)可独立确认 `cost_in_usd_ticks` 这一公共字段存在；它不能证明本Build样本的实际收费、未解释字段含义或新映射。现场结论以保存样本、本地程序及项目契约为准。

## 最小修复与合成对应

1. 仅Build的 `parse_usage`：校验已知完整input+output的checked total；无法推导的total及任意未映射根/详情字段，将**完整Usage object**保留到既有 `Usage.extensions["grok.build.usage"]`。原有严格projector自行拒绝，不新增例外、不忽略0/null/[]、不把原生价格转为账单；四个已知源计数仍可被失败观察器读取。
2. runtime `ProjectedEventSource`：若拒绝事件本身是Usage，缓存已从inner取出的该快照，并经原 `decoded_usage` 接口非加法合并。既有失败flush和source-event幂等账本继续负责持久化；不读后续上游、不重试、不改Attempt outcome。
3. 既有serving管理HTTP回归的132行集成测试触发了基线clippy长度规则；增加仅作用于该测试的具名长度说明，断言/场景未改。新私有probe使用boolean断言避免失败时渲染输入，因此为该probe说明manual_assert_eq例外。初次clippy FAIL保留。

没有修改router的unknown检查、目标codec能力、reasoning策略、原请求、模型选择、持久化schema、管理API、前端或部署。不是让原Chat返回200的修复。

[minimal-native](assets/cpar-native-chat-offline-20261017/minimal-native.json)使用全新合成response/item id、summary和answer，保留两item顺序、summary非空数组/content缺失、type/status/role、空logprobs和annotations、已公开六计数/total。本地程序逐层确认原样本output的字段存在性、null/空值、类型、数组顺序、非空字符串及独立身份关系一致。它是**最小触发fixture**，不是原生root全字段副本。

[usage-extensions-native](assets/cpar-native-chat-offline-20261017/usage-extensions-native.json)补充同类四个Usage字段：已公开字段名保持；未解释私有key改为 `synthetic_cost_details`，非零数值换为合成整数、类型和子结构保持。未知扩展的拒绝与其具体私有key拼写无关。独立plain-body正例是另一受支持输入，不替换原捕获或原正式请求。

## 验证、审查与证据边界

当前已保存：provider RED（丢扩展、非法total被接受）、原样本基线离线重放、真实公共handler的Usage丢失RED、对应修复后GREEN及最终私有白名单重放。原样本最终仍Chat/Messages序号3拒绝；Responses序号8拒绝。所有执行均为LOCAL_CAPTURE_REPLAY或LOCAL_SIMULATED，不是新Provider验收。

公共matrix覆盖两类合成fixture × 三协议 × JSON/SSE，共12组，使用实际生产compiler/factory/lease/auth/handler，仅替换transport。每组独立receive=1，检查安全错误和准入Request ID、Attempt succeeded与Request failed/succeeded的区别、没有伪成功终态；标准Usage下Responses output逐项相等。已知计数均落1个Usage/source-event/ledger，测试主动重复confirmed Usage投递，账本仍1条/cost=null。Responses SSE可先输出可表达summary，后因Usage失败；只对实际error对象检查脱敏，不把已交付语义帧当作错误文本。

负例另覆盖未知item/part字段、重复item身份、错误完成状态、summary空文本结束结构、未知Usage含0/null/[]、非数字/负数/矛盾/溢出total及缺失operand。原gzip有界/无效/叠加encoding、SSE切分与终态身份/顺序回归复用其既有入口，随受影响本地门禁执行。

测试驱动先有缺PathBuf import、误用Messages错误schema、误把Responses正常error:null字段选成错误、误将response.failed内已交付output当作error文本几份FAIL；原日志单列保留。修正的是驱动对已确定codec契约的读取，产品断言不降低；Usage缺失的1→0则是独立产品RED并实际修复。

本批涉及共享公开Usage/failure边界，因此实际执行项目本地Full，结果 **FAIL**：45步中43步PASS，Dependency policy FAIL，最后RustSec audit因fail-stop为NOT_RUN。Rust测试 **1515 PASS / 0 FAIL / 13 IGNORED**；Clippy、格式、Prism双build、真实本地进程/loopback回归、Source policy、秘密扫描、191项契约测试引用及文档链接均PASS。private probe在普通Full中按设计ignored，已在显式本地capture replay单独执行。详见 [local-validation](assets/cpar-native-chat-offline-20261017/local-validation.json)、[Full逐步回执](assets/cpar-native-chat-offline-20261017/local-full-steps.md)和无损压缩保留的 [Full原日志](assets/cpar-native-chat-offline-20261017/local-full.log.gz)。

失败来自本批未更改的Cargo.lock中rustls 0.23.45命中四份现行公告：[RUSTSEC-2026-0346](https://rustsec.org/advisories/RUSTSEC-2026-0346.html)、[0347](https://rustsec.org/advisories/RUSTSEC-2026-0347.html)、[0348](https://rustsec.org/advisories/RUSTSEC-2026-0348.html)、[0349](https://rustsec.org/advisories/RUSTSEC-2026-0349.html)。公告给出的修复版本均为≥0.23.46，但本批未做依赖升级、豁免或部署影响评估；该门禁仍FAIL，后续候选还需独立解决此阻塞。没有用定向绿测覆盖Full失败，也没有无理由重复Full。

最终源码内容绑定见 [tested-source-files](assets/cpar-native-chat-offline-20261017/tested-source-files.json)。**Standards与Spec分别记录；summary/完整Usage/原生模型别名的契约问题、真实矩阵及Full门禁未解决，Spec不得记成无条件PASS。**

## 候选与后续前置

旧ARM64 binary只含bf822b14，**不含本次产品修复**；本批未创建release candidate或manifest。后续候选必须从最终固定新源码构建aarch64-unknown-linux-gnu，记录source、binary SHA、build metadata、manifest及驱动freeze，并重新验证受影响Task/Phase、本地公共matrix及同候选Linux启动/退出/egress/reader兼容门槛；旧Task42/Phase45/系统4证据不能自动覆盖新产品代码。

**下一步唯一优先事项是字段映射/验收语义决策。** 保持当前严格拒绝无需新增Provider请求。若仍要三个协议成功，必须先明确：summary用独立有类型的扩展保留（不能命名为完整body）；额外Usage/未知字段如何无损输出及是否只作为非结算证据；公开模型与原生模型别名如何得到可核验等价关系。这些选择涉及公开表示/消费者及ADR，不在本批自行批准。直接塞进reasoning_content或丢掉Usage会损失语义；仅改用Responses也不能绕过当前额外Usage限制或替代原三协议承诺。

可审查的候选映射如下，**仅用于决策，未实现、未批准**：

| 原信息 | 可选公开扩展 | 信息损失及验收影响 |
| --- | --- | --- |
| 原生output[]完整item/part层级、id、status、顺序及summary/body区分 | Chat `choices[0].message.cpar_native_output`；Messages `cpar_native_output`；Responses保留现有原生output | 保留完整数组可做到JSON语义无损；summary不进入reasoning_content/thinking。消费者必须认识新扩展，不能据此宣称所有标准Chat/Messages客户端等价支持 |
| 完整原生Usage，包括未知key、0/null/[] | 三协议 `usage.cpar_native_usage`；既有 `cpar_usage`继续只含六个已确认计数 | 不解释未知值、不将price字段用于账本；无损传递与结算证明独立。须定义JSON/SSE一致schema、大小界限及拒绝/存储/回放行为 |
| 原生model、时间和其余根metadata/控制回显 | 三协议 `cpar_native_response_metadata`，公开model仍由既有路由边界决定 | 可保留原生值而不伪造模型等价；仍需核对实际模型映射及修改正式oracle的合法依据，不能只删除等值断言 |
| 保持标准目标现有形状 | 不新增字段，继续严格拒绝 | 不发生静默损失，但这类组合仍BLOCKED；若调整交付范围，须用户明确修改承诺，不能自行豁免 #47/#9 |

前三项组合会建立新的有类型扩展契约，并影响所有公共模式及消费者；只有独立ADR/契约与正反例验收完成后才可能将相应输入认定为可表达。本批没有发现可把首个拒绝归为“已有无损映射却错误拒绝”的A类证据。

只有决策、必要实现和新候选门槛完成后，才有理由另行授权真实复测。最小建议预算是新epoch fresh Pin≤1＋**原字节Chat≤1**（总≤2）；不自动转移旧预算。发送前冻结新source/binary/manifest/process/config/Key/route/account/实际lease/model/egress与原请求SHA。待验证假设必须是已实现映射在同一真实形状上能够完整交付；已知必拒而未改变映射时预算应为0。首个FAIL/UNKNOWN/身份或证据矛盾立即永久停止，无retry/fallback/换账号模型/refresh/修复后追加。

GitHub #47/#57/#9当前均OPEN；本批仅授权追加既有progress，原正文/AC/metadata和依赖保留，写后独立读取核对。离线修复不关闭渠道、收口或父规格；远端CI、真实矩阵、发布、部署及生产仍NOT_RUN。

原始测试输出含工具生成的尾随空白。全部13份原日志字节保留在本地及本批首个提交，最终交付另以确定性gzip无损归档，解压逐字节相等；[log-archives](assets/cpar-native-chat-offline-20261017/log-archives.json)绑定原件/压缩件摘要。此证据包装不改写任何RED或Full FAIL，也未重跑Full。

固定 `ec534918→7a876527` 的40文件双轴审查已完成收敛：[审查记录](assets/cpar-native-chat-offline-20261017/review-pre-sync.md)及固定范围/摘要回执。Standards PASS（硬发现0/建议0）；Spec PARTIAL/BLOCKED（新实现违约0/scope creep0，既存契约/真实验收/Full阻塞保留）。初次两路探子的猜测路径错误和PARTIAL均记录，后续收窄补完各自达到complete；没有把初次不完整审查写成PASS。

## GitHub 收口与最终边界

已按审查后的 [追加正文](assets/cpar-native-chat-offline-20261017/github-append.md)，对既有progress [#47](https://github.com/Ricardo121380/cpa-rust-gateway/issues/47#issuecomment-6081215242)、[#57](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57#issuecomment-6081234278)、[#9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9#issuecomment-6081235490)各执行一次body-only PATCH。每次写后独立GET，三票随后再次完整comment/issue/分页依赖读取；正文逐字节等于旧body＋追加正文，历史前缀保持，issue正文/AC/state/labels/assignees/milestone及依赖边/状态均保持。依赖数量8/23/0，OPEN依赖0/11/0；评论编辑导致的updated_at按实际issue读回精确核对。

[同步回执](assets/cpar-native-chat-offline-20261017/github-sync-summary.json)记录PASS3/3，created comments0、closed issues0、issue正文/metadata写入0、push=false。三票均OPEN。新增本地只读绑定回执将实际GitHub响应/载荷摘要保留，以便独立核验；它不包含私有原生响应。

最终核验只针对交付文档、GitHub证据及源文件摘要，不重跑已稳定源码的Full。受测六个Rust文件与固定源码摘要保持，`AGENTS.md`用户修改保持，旧binary/manifest/metadata与所有原始失败仍保留；本批真实Provider调用0、远端新实例0、部署0。诊断和已授权可执行修复已交付；三协议可表达性、真实验收、依赖Full门禁和新候选资格继续BLOCKED，不自动安排下一轮。
