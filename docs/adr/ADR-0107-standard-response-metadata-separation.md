# ADR-0107: Reviewed metadata separation for standard responses

| Field | Value |
| --- | --- |
| Status | Accepted |
| Date | `2026-10-18` |
| Task / Matrix | CPAR-38 #47, CPAR-48 #57, parent #9；标准客户端兼容优先；首条 Grok Build → Responses → 多轮工具调用 |
| Contracts | [ADR-0100](ADR-0100-exact-protocol-semantics-and-usage-evidence.md)、[ADR-0106](ADR-0106-native-fidelity-profile.md)、[BC-ROUTER-004](../contracts/BC-ROUTER-004-protocol-transform-admission.md)、[BC-PROVIDER-003](../contracts/BC-PROVIDER-003-grok-build-responses-boundary.md) |
| Delivery status | 架构方向已确认；字段准入清单、详细契约、实现和验收尚未完成 |

> **同日限定后继决定：** [ADR-0108](ADR-0108-build-summary-standard-messages.md)单独处理
> Grok Build 标准 Messages 的 reasoning summary，可完整加密留存而不展示；它不被重新归类为普通元数据。
> 下文对 reasoning 的一般排除按这一限定例外理解；两个方向均尚未实现。

## Context

用户选择标准客户端兼容优先，目标客户端为 Codex、Claude Code、Pi，并允许按明确支持范围
分版本交付。第二轮问题明确提出：经审查确认不影响回答、工具和标准用量语义的额外元数据，
可以完整加密保留而不在标准响应中输出；未知语义或冲突仍拒绝。用户答复“按照你推荐的来即可”。

当前 Grok Build 完整样本在 Responses 路径因额外 Usage 被拒绝；已知标准计数可以表达，
但额外字段含义与实际模型别名关系仍有未知。已有实现只在内存扩展中保留完整 Usage object，
当前 durable UsageSummary 不含 raw。事实见[原生离线定位](../reports/cpar-native-chat-offline-20261017.md)
及[字段决策与依赖报告](../reports/cpar-field-contract-dependency-20261017.md)。

ADR-0106 的 opt-in `native-v1` 将完整原生信息交付给专用消费者，并保留默认严格路径。
本次决定增加一条有严格适用条件的标准路径规则；不以专用消费者作为标准客户端可用的前提。

## Decision

### 标准响应与加密证据的分工

标准客户端可以收到独立成功的标准响应，同时由 CPAR 完整加密保存**经审查的可分离元数据**。
该规则不要求标准客户端实现 `native-v1`，也不向其自动加入 native-v1 自定义事件。
标准响应成功不宣称调用方已经收到完整原生封套或整包 wire 保真。

准入必须逐字段、逐适用范围明确：字段及值形状、来源渠道、协议/模式、适用条件、证据依据、
标准语义不受影响的理由，以及保留和读取方式。字段位于 Usage 下、名字像价格或能够解析为 JSON，
均不足以证明它可分离。**本 ADR 没有批准任何实际未知字段；当前未解释 object 仍不获准。**

下列边界保持：

- 回答、工具声明/调用/结果关联、历史顺序、终态和错误、请求控制不能作为可分离元数据移走。
- reasoning summary/body、item/part 身份和内容语义仍按原有证明转换；不能把 summary 当 body，
  也不能用本决定解除 Chat/Messages 的 summary 表达阻塞。
- 已知源计数、计数来源与口径、标准 Usage 的精确映射不减少；缺失保持未知，冲突继续拒绝。
- 模型身份和别名证明要求继续有效，不能把身份矛盾移入证据后宣称成功。
- 尚未证明语义、未准入的字段/形状和任何冲突继续拒绝；不采用通配忽略或任意 opaque 放行。

### 完整保留是成功条件

分离后的信息须按 exact Request/Attempt 和确切 owner 完整加密留存，可由获授权的诊断路径核对。
保留缺失/null/0、原值、结构及精确数字；不通过有损解析、截断或日志替代持久化。
使用现有身份、AEAD、保留期限和幂等边界；详细存储与 reader 契约在实施前明确。

对使用本规则取得成功的请求，所承诺证据持久化和标准投影核对必须先完成，之后才能确认成功终态。
写入失败或证据冲突不能给出该规则下的成功；SSE 已提交 HTTP headers 时按流失败收口。
失败与取消继续保存已验证的 Usage，不读 ahead、不额外重试，也不制造第二份账本。
诊断原文不进入普通日志、审计、Git 或模型输入。

原生价格信息属于非结算证据。真实 Provider 费用仍须独立证明，不能把它直接变成账单或补造零费用。

### 与既有决定的关系及生效范围

本 ADR 限定修订 ADR-0100/0106 中“信息不能在标准响应完整交付时必须拒绝”的要求，
仅对满足上述准入、语义与完整持久化条件的元数据增加例外。其他严格转换、所有权和费用规则不变。
`native-v1` 的完整消费者交付及其验收义务继续保留；两条路径分别声明支持、分别验收。

当前优先工作从 Grok Build → Responses 开始。其他渠道、协议和模式不能因共享代码自动获得准入；
首版必须同时通过 Codex、Claude Code、Pi，三者均调用 Grok Build；工程侧先选可复现 CLI 样例。
Messages 的 reasoning summary 交付仍需独立解决，不能把它当作本 ADR 已批准的元数据分离。
此处不改变 Passthrough 的 exact 含义，也不新增转换模式。
实施批次须明确更新相关 BC 和准入/输出/存储规则，不能只删除 projector 的扩展检查。
**当前代码仍执行原严格拒绝；文档 Accepted 不代表规则已经实现或任何样本已经通过。**

## Consequences

标准客户端可用性不再要求其消费完整原生扩展，但实现必须承担字段审查、加密持久化及诊断责任。
现有样本的未知字段与模型关系仍可能阻塞实际交付。Codex、Claude Code、Pi 必须按实际协议与形态
分别验收，不能由一个 Responses 成功推出三个客户端都兼容。
版本支持范围可分批扩大；#47/#57/#9 的剩余承诺继续保留。

## Alternatives considered

- 保持所有额外信息必须交付调用方，否则拒绝：语义边界清楚，但已证明无关的元数据也会阻塞标准客户端。
- 经审查的元数据分离并完整加密留存：用户选择；标准语义保持，增加证据持久化与诊断义务。
- 丢弃未知字段、假定不影响语义，或强制普通客户端消费原生扩展：未获选择，不能据本 ADR 实施。

## Validation and rollback

实施验收需通过实际公共路径覆盖：获准元数据的标准成功、完整加密证据核对、已知 Usage 保留，
以及未知/冲突/精度丢失/写入失败的拒绝；SSE 增量和唯一终态、工具续轮、取消、owner 隔离分别验证。
具体客户端执行、真实 Provider、候选和部署证据分列；本批仅记录决定，没有行为测试结果。

迁移与 reader 兼容性必须随实现证明；新证据写入后不能假定仅回退 binary 即安全。
旧记录保持原来的 UNKNOWN，旧 FAIL 与 stopped epoch 保留。本文不授权真实调用、远端写入、发布或部署。
