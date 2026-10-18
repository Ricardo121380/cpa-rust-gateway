## Parent

[产品规格 #58：CPAR Rust 网关目标与三客户端首版范围](https://github.com/Ricardo121380/cpa-rust-gateway/issues/58)

## What to build

在获准的 Grok Build → Messages 路径中，JSON 与 SSE 均能交付回答和多轮工具调用；完整摘要及身份顺序加密留存但不向客户端展示，并证明续轮连续、两种传输模式语义一致。

覆盖 #58 用户故事 11, 16, 17, 18, 19, 20, 22, 23, 24, 27, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 40, 41, 42, 43, 44, 45, 56；验收场景 T02, T03, T04, T05, T06, T07, T08, T09, T10。

本票承接 #58 的已确认范围；实际开工须有用户明确指令。真实账号、收费请求、共享服务与发布部署分别遵守既有授权边界。

各验收组必须全部满足；可分别记录进展，任一必需组受阻时整票不能标完成。

## Acceptance criteria

### Messages JSON

- [ ] 在#61的获准元数据、证据和模型边界上，贯通公共 Messages JSON、生产 Build 装配、摘要校验、加密保存、标准响应与授权读回。
- [ ] summary 及其 item/part 身份、层级、顺序完整保留；缺失 body 保持缺失。不拼入回答、不伪装成完整 thinking、不伪造 signature。
- [ ] 在最终限定交付策略与现有转换模式间明确契约；不以 User-Agent 跳过规则，不将省略摘要宣称为完整 LosslessBridge，不改变 Passthrough exact。
- [ ] 以两次工具循环、结果回传与后续问答证明所测续轮路径连续；如果模式依赖未保留的上游摘要/身份，则该模式仍拒绝，不能用已存数据库代替续轮证明。
- [ ] Messages 已知不兼容的请求控制或历史在发送前准确拒绝，独立受控接收数为零；响应之后才可发现的冲突则准确失败且不重发。
- [ ] 未知 part、空摘要相关非法状态、重复/冲突身份、done/terminal 矛盾和写入失败都保留准确失败；其他 reasoning、密文、签名、引用不自动取得省略资格。
- [ ] 回答、工具、标准 Usage 与模型身份正确；额外 Usage 继续逐字段准入，成功响应前已确认完整加密保存。
- [ ] 公开支持说明注明此路径不显示 Grok 思考摘要；Responses 仍保留 summary，Chat/其他 Provider 不继承例外。此票为受控协议行为，Claude Code 实际兼容另验。

### Messages SSE

- [ ] 复用本票 JSON 路径的摘要、元数据、模型与模式契约，贯通公共 Messages SSE 和生产 Build 装配；summary 仍完整解码与校验，仅按已确认规则不进入客户端展示。
- [ ] 验证合法 message/content-block 生命周期、增量文本、分片工具参数和唯一终态；两个工具循环的 ID、参数、结果与后续回答准确关联。对同一合法上游内容，Messages SSE 与 Messages JSON 的回答、工具关联、完整 summary 留存及已知 Usage 语义一致。
- [ ] 完整加密证据与标准投影确认先于成功终态；未知内容、摘要冲突、加密/写入/确认失败不得产生 message_stop 等成功收口。
- [ ] 取消、断连、超时及上游畸形事件停止工作，保留已经验证的 Usage，不额外读 ahead、重发或重复计费。
- [ ] 受控上游独立计数、授权证据读回和 Request/Attempt/Usage/账本核对一起证明结果；已知不兼容请求在发送前拒绝并验证接收数为零；JSON 和 Responses summary 保留行为回归。
- [ ] 本票覆盖流式协议行为，不借此宣称 Claude Code 实际客户端或真实渠道已经通过。

## Blocked by

- #61：标准 Responses 的 JSON、SSE 与工具闭环
