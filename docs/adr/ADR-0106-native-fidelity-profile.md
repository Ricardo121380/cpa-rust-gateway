# ADR-0106: Negotiated native fidelity for three public protocols

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-10-17` |
| Task / Matrix | CPAR-38 #47, CPAR-48 #57, parent #9; T08–T12, T14–T15 |
| Contracts | [ADR-0100](ADR-0100-exact-protocol-semantics-and-usage-evidence.md), [ADR-0101](ADR-0101-grok-native-item-metadata-fidelity.md), [ADR-0105](ADR-0105-projection-failure-observation.md), [BC-ROUTER-004](../contracts/BC-ROUTER-004-protocol-transform-admission.md), [BC-PROVIDER-003](../contracts/BC-PROVIDER-003-grok-build-responses-boundary.md) |
| Companion | [Acceptance matrix](../reports/cpar-field-mapping-acceptance-20261017.md), [envelope schema](../reports/assets/cpar-field-contract-dependency-20261017/native-fidelity-v1.schema.json) |

## Context

用户已于本批选择下述架构方向；决定为 Accepted，字段级 schema/验收细则作为下一批实施契约草稿，
没有对应行为修改。当前严格拒绝继续有效，新 profile 未获得实现或 capability 资格。
原生 summary 与完整 reasoning body 的区别、额外 Usage、原生/public 模型不同名已经定位；
本批复用已保存合成 fixture 和结构摘要，没有重新捕获或向模型输入私有原文。
上一报告中的 `cpar_native_output` 等字段名是候选，不构成设计批准。

标准表示的可表达性如下。这里的“标准”指官方协议槽位；本仓 Chat `reasoning_content`
是既有兼容扩展，不能称 OpenAI 标准的 summary 槽位。

| 原生信息 | Responses | Chat Completions | Messages |
|---|---|---|---|
| reasoning `summary[]` / `content[]` | 两组独立有序 parts；`summary_text` / `reasoning_text` | 官方 message/delta 没有独立 summary、body、item/part 层级 | 可见 thinking 和 signature 有自己的含义，不能证明与 Build summary/body 等价 |
| output item ID、part field/index、跨 item 顺序 | native output 和原生事件可承载 | choice/tool-call 索引只承载其已证明的范围 | content block 索引只承载其已证明的范围 |
| 六个已知源计数及 evidence | 标准 usage＋既有 `cpar_usage` | 同左；按输入口径做精确算术 | 同左；inclusive/exclusive 严格换算 |
| 完整 native Usage / 未知 key / 显式值形状 | 公共标准 schema 不能覆盖全部 Build 字段 | 不能覆盖 | 不能覆盖 |
| public/upstream/native model 与 root metadata | 一个 model 字段不表达三个角色及等价证明 | 同左 | 同左 |

官方来源在本批实际读取：[OpenAI reasoning](https://developers.openai.com/api/docs/guides/reasoning)、
[Chat streaming schema](https://developers.openai.com/api/reference/resources/chat/subresources/completions/streaming-events)、
[Anthropic thinking](https://platform.claude.com/docs/en/build-with-claude/thinking)、
[xAI Responses](https://docs.x.ai/developers/rest-api-reference/inference/responses)、
[xAI cost tracking](https://docs.x.ai/developers/cost-tracking)、
[xAI aliases](https://docs.x.ai/developers/models)。这些来源证明表示和官方 API 规则，
不能证明本 Build 样本的收费或三个实际模型名的等价。

固定 SDK 的实际离线消费者探测见 [consumer results](../reports/assets/cpar-field-contract-dependency-20261017/consumer-results.json)。
OpenAI Node 7.36.0、Anthropic Node 0.135.0 的 JSON 解析保留未知字段；Chat 流 helper
覆盖未知片段为最后一个，Messages helper 不累计新增的末态 Usage 字段并跳过自定义事件，
Responses helper 遇到自定义事件拒绝，移除该事件的对照则成功。
这七个内存 fetch 场景不调用 Provider，也不证明其他版本、严格生成客户端或产品消费者兼容。

## Decision — confirmed direction, implementation pending

采用显式协商的 `native-v1` fidelity profile，保留默认严格模式；完整原生 Usage
作为非结算证据保存；模型验收允许字面同名或有范围的独立别名证明。
用户已选择扩展交付与非结算 Usage 证据、有证据别名规则。未实现/未证明部分继续 BLOCKED；选择扩展不等于确认未知字段含义、
别名或真实费用，也不自动调整 #9 完成标准。

### Profile admission and consumers

以下名称都是本 ADR 的具体提案。请求头为 `X-CPAR-Fidelity: native-v1`；仅允许一个精确值，
未知版本、重复头、列表值、未注册实现及不支持的模式在获得 lease 前拒绝。响应确认同名同值。
没有该头时仍按现有标准/严格路径处理；客户端不能被静默升级。

初始范围仅为 `grok.build` 的三协议 JSON/SSE；每个模式需要自己的 capability/实现证明。
Canonical、LosslessBridge、CanonicalBridge 仍满足各自拓扑与等价证明。Exact Passthrough
不能加封套后仍称 exact，因此该 profile 与 Passthrough 的组合先拒绝。
Console、Official、其他渠道和其他能力不因共享代码自动获得声明。

三协议 JSON 都使用根字段 `cpar_fidelity`，固定闭合 schema `cpar.native-fidelity.v1`。
该字段是响应原生信息的权威视图；既有标准 answer/tool/usage 是经核对的投影，不能与之矛盾。
Responses 的标准 `output` 与该视图重叠时逐项核对，不生成第二套相互独立的事实。

消费者必须：确认 profile；使用精确数值解析；验证 schema、原生字段与投影一致性；
按 ID/field/index/order 还原；处理扩展 SSE 事务；先确认扩展完整再承认成功；历史回传保留原生身份。
仅能读到未知 JSON 字段而未履行这些要求的客户端，不获得 fidelity PASS。
SDK 默认 UI、thinking helper、Usage helper 不会因此自动消费新字段。

### 1. Reasoning, identity and order

| 字段 | proposed mapping / invariant |
|---|---|
| `output[]` 顺序 | `cpar_fidelity.output[i].output_index=i`；`item` 为已审查 native message/reasoning/function_call 形状 |
| item `id/type/status/role/call_id/name` | 原值与 presence 保留；不得把缺失 status 改写为来源报告的 completed；另由生命周期证明有效完成 |
| reasoning `summary[]` | 仍叫 summary，type=`summary_text`，按原数组顺序保存；不写入 `reasoning_content`/`thinking` |
| reasoning `content[]` | type=`reasoning_text`，只保存实际收到的 body；缺失与[]分开，不合法null仍拒绝，reasoning_tokens 不生成 body |
| part identity | `(item.id, field=summary|content, index)`；没有 native part ID 时不制造来源 ID |
| observed semantic order | `semantic_order[]` 保存 gateway ordinal、kind、item/part 地址；原生 sequence 有值才另存十进制串，不伪装为原生序号 |
| annotations/logprobs/phase | 沿用已审查 schema，保留原生 presence/null/[]；不能转换成伪造 Claude citation/signature |
| cipher/signature | 外来非空密文或未建立 owner/decryption 的内容继续拒绝；本草稿不增加解密或签名路径 |

JSON 的原数组有序；JSON 对象成员顺序不作为语义顺序。JSON decoder 生成的 semantic ordinal
不是真实 SSE 到达时序。SSE 保留实际解码的语义顺序与 source sequence 的有无；同一源的
JSON/SSE 必须得到相等的最终字段/层级/身份，不宣称两种源有相同原始帧或时序。
原生 sequence 若提供须在同一响应内严格递增；缺失仍为缺失，重复/倒序拒绝。

item start/delta/done/terminal 必须相互确认：索引不能改指另一个 item，已封口 part
不能再增量，done 必须确认先前片段，terminal 必须确认 item 集与顺序。summary、body、answer
互不补齐。未知 item/part 语义字段仍拒绝，不能藏入 opaque 包绕过现有闭合校验。

Chat/Messages 的标准 answer/tool 保留现有已证明投影。多 part reasoning 的权威版本在扩展；
不能通过把它们拼成一个 plain reasoning body 取得成功。已有单一 plain body 的兼容投影
只在当前已证明子集内使用，并核对扩展内容。

### 2. Complete Usage and accounting

`cpar_fidelity.usage` 是判别联合：`{presence:"absent"}`、`{presence:"null"}`，
或 `{presence:"object",raw_json:"<complete native Usage object>"}`。
raw_json 必须直接保留有界原生片段，严格检查 JSON/重复 key 后保存；不能先经浮点
Value/to_string 往返。这避免未知大整数被普通 JS `JSON.parse` 改值，亦保留 0/null/[]/{}、
未知嵌套 key 和数字词法。JSON Schema 只校验封套；内层 JSON 与字段语义由明确解析规则校验。

| 信息 | 公开 / 耐久 / 账本规则 |
|---|---|
| six nullable counters / provenance / input_accounting | 既有 `cpar_usage` 和 source-event 不变；仅按已知口径精确转换，缺项仍 unknown |
| `total_tokens` | input/output 都已知时用 checked u64 arithmetic 验证；缺 operand 不制造值，原 supplied total 仍在 raw_json |
| known counter `null` / absent / explicit 0 | 标准 optional 计数可以归一化为未知；原生 raw_json 分别保留，0 不等于未知 |
| unknown root/details / empty arrays/objects | 原值与层级保留，不加总、不变成标准 token count；该原生证据没有新的已知语义承诺 |
| snapshot updates | 按确切 Attempt 非加法合并已验证计数；最终快照确认此前合法观察，冲突拒绝；不跨 Attempt 取最大/相加 |
| failed Usage projection / failure / cancel | 只保存已经解码验证的证据；不读 ahead、不重试；已有一条 Usage/source-event/ledger 幂等规则保持 |
| native price-like fields | 原文可作为证据，不能成为费用或 quota；真实 charge 仍 UNKNOWN，ledger 不回填未经证明价格 |

xAI 官方 API 将 ticks 描述为美元的 10^-10 单位并给出费用规则；它不能证明 OAuth Build
样本的单位、结算归属、折扣或独立账单。价格单位声明、上游报价、catalog 计算费用与实际
Provider charge 分开。Measured counters 也不足以独立证明最终 charge。

**持久化实施要求：**现有 `UsageSummary` 有意不携带 protocol extensions，不能仅改
公共编码器就宣称完整耐久保真。推荐新增有版本的加密 native evidence 记录，绑定确切
Request/Attempt 和现有 source-event identity；失败 Attempt 也能保存已验证 Usage。
记录单独保存 raw Usage / root evidence，ledger 只引用原有合法计数；不把原文加到事件日志、
审计、遥测或普通计费 payload。重复 identity＋相同证据幂等；同 identity＋不同证据是冲突，
不能覆盖或新建第二条账本。存储不可用时按已声明的耐久要求失败，不能给完整证据 PASS。

需要单独版本化 migration、AEAD domain/AAD（Client Key owner＋lineage＋record version）、
旧数据 unknown 默认、保留 key versions、TTL/删除/故障/restart 及旧 reader 兼容证明。
未来旧二进制单独回退不得默认安全。这里没有实施数据库迁移。

### 3. Models and native root metadata

公开 `model` 继续来自 route/public-model 解析；上游 `model` 来自实际 selected candidate。
Native root model 只记录收到的值，不用 public 名覆盖。profile 的 `model_binding` 为：

```json
{"public_model":"synthetic-public","upstream_model":"synthetic-upstream",
 "native_model":{"presence":"value","value":"synthetic-native"},
 "relation":"unknown"}
```

native_model 也允许 presence=`absent` 或 `null`。relation 是 `literal`、`attested_alias`
或 `unknown`；只有 `attested_alias` 带 `evidence_sha256`。字符串相同仍须与路由和实际执行
身份相互核对；不相同必须有独立证明，否则 UNKNOWN/BLOCKED。

`metadata.raw_json` 保存 native root 中除 output/usage 外的已准入字段对象，包含 native model、
原生时间、status、metadata、控制回显、service/store 等的原值/presence。公开创建时间仍按
既有 route/codec 契约提供；不能用它冒称上游时间。映射时分别记录两个时间来源。

初始允许 root 字段表：`id, object, model, created_at, status, error, incomplete_details,
instructions, max_output_tokens, parallel_tool_calls, previous_response_id, reasoning, store,
temperature, text, tool_choice, tools, top_p, truncation, metadata, service_tier, user`。
字段出现不意味着控制或 capability 已获支持，仍须现有请求/响应语义校验。
metadata 内的任意 JSON 只能作为来源回显而不触发控制；未知 root key 必须先扩充闭合准入
规则并审查，否则拒绝。不得自动公开未知敏感 root；安全摘要中未解释 key 只用既有 hash。
本表是 Proposed 的最大候选集合，未覆盖/未验证字段逐项待实现，不能据表自动放开 decoder。

正式验收的推荐三段断言：

1. request.public == 当前 route/public model；actual dispatched upstream == frozen selected upstream。
2. native model 与 selected upstream 的关系，要么字面相等，要么匹配**预先冻结**的可信别名证据。
3. public response.model == public model；扩展 native model == 已验证原生值；证据适用于 exact
   channel/model/config/版本/有效时间，权限和能力独立校验。

别名证据来自明确适用的官方版本声明/可信 serving declaration 或另行批准的验证材料，
须绑定源 URL/hash、三个具体名、渠道/profile、适用版本及失效条件。单次响应、名称相似、
public catalog 或 generic xAI `-latest` 规则不能独自证明本 Build 映射。
没有证据就不能选择另一模型、绕过 assertion 或把 UNKNOWN 写成 PASS。

旧 `verify_success_fidelity` 在冻结 driver 中要求 native/public 都字面等于 grok-4.5；
保持原文件和失败。本草稿批准后也只会建立带反例的新 oracle，不删除旧断言来制造成功。
本次关系仍 UNKNOWN；源名原文没有进入本 ADR。

### JSON, SSE, terminal, storage and replay

JSON 返回完整 schema 封套。SSE 使用扩展事务：`cpar.fidelity.start` 声明 version，
`cpar.fidelity.delta` 携带按 UTF-8 字符边界切分的封套 JSON 字符串片段和递增 ordinal，
`cpar.fidelity.complete` 携带片段数、UTF-8 字节数、SHA256。消费者拼接后严格解析同一个 JSON
schema，与 JSON 模式得到相同字段值/层级/身份/最终状态。SHA 只证明一致性，不是签名或别名证据。

该事务可以在已经解码的证据可用时开始；不要求读 ahead。最终封套完整且与标准投影相互确认后，
才能发标准成功终态：Chat finish＋Usage＋`[DONE]`、Messages最终 delta＋stop、Responses completed。
失败只发已有安全 error，绝不 complete/成功标记；HTTP headers 已提交后仍是 SSE failure，
不伪造新的 HTTP502。专用消费者可以显示 provisional 标准 deltas，但不能提前确认完整保真成功。
未知扩展事件不能直接交给未经适配的 SDK helper；探测已证明部分 helper 会拒绝/跳过。

这使最终完整语义一致，但不保证 summary 与 standard text 同时到达，也不保证原始 SSE 帧的
字节/网络分段时序等价。低延迟逐 part 扩展事件属于以后独立变化，不能在本方案中暗示已交付。

成功 StoredResponses 保存同一封套/Canonical 证据和 profile version，完成 encode 和加密
持久化后才发送成功终态。GET/WS/compact/restart 必须读取并确认 exact owner/TTL/lineage；
不支持版本的 reader 或缺 profile 的读取，明确拒绝并保留数据。当前仅三协议 JSON/SSE 草案，
store/continuity/compact/WS 每项仍需独立实施和能力资格，基础 PASS 不自动给扩展 PASS。
历史回传原生 item/part/tool 身份和顺序；禁止把 native metadata 回显误当成下一轮控制，
也不能向别的 Provider 回放任意 opaque root。跨 owner/channel/profile 的回放拒绝。

### Bounds, unknowns and conflicts

| 范围 | Proposed upper bound / rule |
|---|---|
| source Build JSON | 原压缩/解压各 1 MiB、原 SSE input record 64 KiB，保持现有更小边界 |
| output | ≤256 items；每 summary/content ≤64 parts；ID/name ≤256 UTF-8 bytes；累计保留文本≤1 MiB |
| annotations | indices≤64；累计 payload≤1 MiB；未知结构/非空不支持 logprobs 拒绝 |
| raw Usage / root metadata | 分别≤64 KiB / 128 KiB（UTF-8 原生片段）；嵌套 depth≤32；重复 key拒绝 |
| semantic_order | ≤4096 observations；ordinal从0递增，无别名/改指；native sequence为精确u64十进制串 |
| emitted envelope | 序列化 UTF-8 ≤8 MiB，包含 escaping 与重复标准视图；JSON/SSE同一总上界 |
| profile SSE record | encoded frame≤64 KiB；按实际转义后大小切分、UTF-8不拆字符；总片段≤2048 |
| extension schema | 外层未知字段/未知版本拒绝；raw Usage未知值仅保留，known值仍严格验证；root/part未知语义拒绝 |
| storage | 原 StoredResponses 8 MiB / 4096事件更小者优先；不能因profile扩大既有边界 |

Schema 的 maxLength 是字符数，不能替代这些字节界限。源边界、aggregate、depth、身份、
field presence、raw_json、投影一致性和 lifecycle 校验都必须执行；不能只跑 JSON Schema。
任何 limit/字段冲突产生原有安全错误和已验证 Usage flush；不截断、不猜优先级、不 fallback。

## Consequences

完整原生信息的交付会成为协商的 CPAR 协议扩展和专用消费者义务。默认标准路径仍严格。
#9 的三协议承诺没有被自动豁免：基础标准消费者矩阵与 negotiated fidelity 矩阵分列，
直到用户明确决定 extended profile 能否计入哪些交付项，否则 #47/#57/#9 整体继续 BLOCKED。
本 ADR 的选择接受扩展作为 rich 语义的交付形式；不能证明标准客户端完整兼容，
标准矩阵和其他 #9 承诺仍必须完成。库探测不能代替产品/真实渠道验收。

Usage 加密记录、原生 root 保留、SSE reducer、reader/migration 和新模型 oracle 都是实际工作，
不能只添加三个 JSON 字段就宣布支持。公开数据只交给确切授权消费者；原文不进日志、Git、
GitHub 或模型输入。本批文档与 fixture 全部使用合成值/既有安全摘要。

## Alternatives considered

| 方案 | 评价 |
|---|---|
| 继续默认严格拒绝，等待标准/原生可表达证明 | 保持正确；该组合继续 BLOCKED；可选择但不完成三协议目标 |
| 本草稿：opt-in完整证据＋专用消费者＋有证据的模型关系 | 推荐；不丢字段，能逐层验收，但公开profile/消费者/持久化有额外义务 |
| 默认给所有客户端加扩展并宣称完整兼容 | SDK实测反例、缺消费者语义证明，拒绝 |
| summary当body、删未知Usage、价格直接入账、删除模型assertion | 违反严格等价/成本/身份要求，拒绝 |
| 只用Responses替代原三协议交付 | 不能替代 #9；完整Usage和模型关系也没有因此解决 |

## Validation and rollback

逐字段成功/拒绝、JSON/SSE、成功/失败/取消、持久/回放和消费者证据见 companion matrix；
未来测试以 actual public compiler/factory/handler 和独立 controlled receive count 为主，
现有合成 native fixtures 复用。本批当前语义回归由依赖提交的 Full 覆盖；新 profile 尚未实现。
真实 Provider、fresh Pin/Chat、远端实例、候选、CI、部署、生产没有本 ADR 授权。

下一批先固定 schema/字段表/消费者承诺并增加 BC、迁移/reader qualification，再实现最小
公共路径与必要回放。读旧记录仍unknown；新证据不能改写旧FAIL/旧账本/旧预算。
若写入新版加密证据，只回退 binary 不保证可读；按 ADR-0100/0103 保留历史/key并验证
兼容 reader 或经另行授权的 coherent restore。草稿本身没有运行状态可回退。

## Decision record

1. **公开表示/消费者与 Usage：**用户选择 opt-in `native-v1`＋专用消费者、完整 raw Usage＋
   按Attempt加密证据、未知字段非结算，按草稿进入下一批实施；默认严格路径保留。
2. **模型规则：**用户选择字面等值或预先冻结的有范围可信别名证明＋三段断言。
   本样本未有证明，实际关系仍 UNKNOWN/BLOCKED。
3. 同次用户选择另行授权 `quinn-proto 0.11.18` 补丁与新版 Full；这是本批依赖范围的追加，
   与协议实现/Provider预算分开。记录见 [decisions](../reports/assets/cpar-field-contract-dependency-20261017/decisions.json)。
