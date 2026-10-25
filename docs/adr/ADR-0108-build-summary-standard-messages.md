# ADR-0108: Grok Build summary retention for standard Messages delivery

| Field | Value |
| --- | --- |
| Status | Accepted |
| Date | `2026-10-18` |
| Task / Matrix | CPAR-38 #47, CPAR-48 #57, parent #9；首版 Grok Build → Messages → Claude Code 多轮工具调用 |
| Contracts | [ADR-0100](ADR-0100-exact-protocol-semantics-and-usage-evidence.md)、[ADR-0101](ADR-0101-grok-native-item-metadata-fidelity.md)、[ADR-0106](ADR-0106-native-fidelity-profile.md)、[ADR-0107](ADR-0107-standard-response-metadata-separation.md)、[BC-ROUTER-004](../contracts/BC-ROUTER-004-protocol-transform-admission.md) |
| Delivery status | 产品交付边界已确认；详细契约、实现、持久化和工具续轮证明待完成 |

## Context

用户要求首个可用版本同时通过 Codex、Claude Code、Pi，三个客户端均调用 Grok Build。
Claude Code 的常规自定义网关使用 Messages。当前 Build 样本的 reasoning summary 与完整 body
是不同概念，Messages 尚无已证明等价的映射；已有严格规则据此拒绝。
ADR-0107 仅允许经审查的普通元数据分离，没有授权移走 reasoning 内容。

用户在第 9 题单独选择：首版允许 Claude Code 不展示思考摘要，CPAR 完整加密留存，
确认不影响工具续轮后，标准调用可以成功。该选择明确改变下游展示范围，不能伪装成全信息等价转换。

## Decision

### 限定的标准交付范围

对经过实现与验收资格确认的 Grok Build → 标准 Messages 路径，允许 reasoning summary
不进入客户端显示内容。CPAR 必须完整加密保存该摘要及其原生 item/part 身份、层级和顺序，
供授权诊断核对；缺失的 reasoning body 保持缺失。
回答、工具调用/结果、标准用量、错误与终态仍按原契约完整交付。

这是明确的标准客户端交付范围，不能宣称该客户端收到了完整原生 reasoning。
支持说明必须可见地列出“此路径不展示 Grok 思考摘要”；不以 User-Agent 名称临时跳过校验。
无需普通客户端理解 native-v1 封套，但也不能给它伪造 `thinking`、signature 或完整 body。

### 成功的必要条件

- 摘要仍经完整 decoder/lifecycle 校验；未知语义、身份/顺序冲突、done/terminal 矛盾不能因不展示而略过。
- 摘要及其关联证据按确切 Request/Attempt/owner 完整加密持久化并可核对后，才允许成功终态；
  写入失败、精度或内容丢失均不得成功。失败与取消按已验证范围保留证据，不额外读取或重发。
- 公共 Messages 请求、真实客户端工具调用、结果回传、后续问答必须证明语义连续；
  不展示摘要不能导致工具 ID 丢失、上下文缺失或后续轮次产生不同的已承诺行为。
- 若某种上游模式的续轮依赖该摘要或原生身份，必须有经过验证的保持机制；
  尚无机制时该模式继续拒绝，不能用“摘要已存在数据库”代替实际续轮证明。
- 请求端 thinking/budget、输出上限、并行工具及其他控制继续按能力与等价规则准入；
  不通过删除参数、近似转换或降低能力声明，使客户端看似成功。

仅限已审查的 summary 表示。完整 reasoning body、密文、签名、未知 part、引用、模型关系
不因本决定自动获得省略/转换资格。额外 Usage 仍单独满足 ADR-0107；未知费用不计入账单。

### 与既有决定的关系

本 ADR 对 ADR-0100/0101/0106 的严格 summary 交付规则作限定修订，仅适用于上述
Grok Build 标准 Messages 路径；ADR-0107 排除 reasoning 的一般规则由此获得这一单独批准的例外。
其他严格语义边界、Provider 隔离、认证、路由、重试及所有权规则继续有效。

Responses 继续交付其可表达的原生 summary；Chat 不自动取得此例外；其他 Provider 不继承。
`native-v1` 的完整原生交付与专用消费者承诺保留，标准路径通过不替代其验收。
本次只确认交付取舍；当前代码仍拒绝原组合，详细 BC、存储与回放资格随实施补齐。
现有转换模式若要求全信息等价，实施时须明确其与本限定交付策略的关系；
不能因本 ADR 存在就把省略客户端摘要的路径标成已证明的 LosslessBridge，Passthrough 的 exact 含义不变。

## Consequences

Claude Code 首版可以在不展示 Grok 思考摘要的情况下完成标准回答和多轮工具任务，
前提是加密证据与续轮证明都满足。工程需同时交付转换、诊断留存和客户端闭环，
不能仅过滤 ReasoningDelta 后把测试改绿。实际模型关系和未知 Usage 仍是独立阻断。

该限定支持范围在版本说明中公开；历史严格模式的 FAIL 继续保留。
标准 Messages、Responses 和 native-v1 的结果分别记账，不自动关闭 #47/#57/#9。

## Alternatives considered

- 首版必须展示摘要：等待或设计可靠的标准表达；用户没有选择作为首版门槛。
- 不展示摘要、完整加密留存且证明续轮不受影响：用户选择，接受展示范围限制和证据维护成本。
- 摘要拼进回答、伪装成完整 thinking、伪造签名或直接删除：没有被本决定授权。

## Validation and rollback

以真实公共 compiler/factory/handler、受控上游和实际 Claude Code 验证多轮工具闭环，
同时核对独立接收数、标准输出、完整加密摘要、已知 Usage、唯一终态和账本身份。
保留摘要冲突、未知形状、持久化失败、取消、错误归属和依赖未满足的反例；Responses 的原生保留须回归。
在相同候选上分别报告三个客户端的结果，真实 Provider 与本地受控证据分开。

存储布局与 reader/迁移沿用受审查方案，必须证明可读性及恢复条件；
新证据写入后不能假定只回退 binary 安全。当前没有行为修改，没有新增真实预算、发布或部署授权。
