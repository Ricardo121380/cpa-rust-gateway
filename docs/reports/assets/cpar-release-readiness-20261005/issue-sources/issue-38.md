# #38 CPAR-29：Responses WebSocket 生命周期与续接

State: OPEN; Updated: 2026-09-29T13:32:01Z

## Parent

[实施规格 #9：CPAR 现有渠道可靠性与管理体验收敛](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9)

## What to build

WebSocket 多轮调用沿用同一工具、取消、用量和所有权边界，保持连接内及适用持久历史的精确连续性。

实施编号 CPAR-29；实施批次：B。覆盖用户故事 58, 59, 63, 66, 67, 68, 72, 73, 74；验收场景 T10, T11。

## Acceptance criteria

- [ ] 真实 loopback 握手验证认证、现有 Origin 规则、response.create 事件及下游 framing；不是 Realtime，不强制上游 WebSocket。
- [ ] 复用 Canonical 执行，文本及已声明工具／思考语义与 HTTP／SSE 一致，不另建调度、重试或计费路径。
- [ ] 连接内多轮及适用持久续接保留精确 lineage，失败轮次不成为成功根，不隐式替换账号。
- [ ] 取消、关闭、超时及写失败终止实际上游并释放资源，保持既有队列、大小、分片、时间和关闭码边界。
- [ ] 成功存储顺序和历史回传不丢语义；声明能力与实际准入一致，不因下游 WS 宣称 Provider 原生 WS。
- [ ] 记录受影响行为的高层本地验证、必要的 EgoLite 产品验收与审查证据；静态存在、fixture、真实渠道和生产结论分开，结果使用 PASS／FAIL／NOT_RUN／BLOCKED。
- [ ] 保留账号／渠道／权限及数据边界；涉及权威管理契约时单向同步生成客户端，遵守既有前后端归属与跨边界记录；必要 ADR 与相关陈旧测试入口随本切片修正。
- [ ] 在实际 checkout 上复核范围，完成适用 Task／Phase 门槛；本票不自动授权真实账号、收费请求、服务变更或部署，具体外部操作以已明确目标和范围为准。

## Blocked by

- [#29](https://github.com/Ricardo121380/cpa-rust-gateway/issues/29)：流式工具与并行控制保持完整。原因：复用正确的流式工具与并行语义。
- [#33](https://github.com/Ricardo121380/cpa-rust-gateway/issues/33)：流终态、取消与透明重试正确。原因：复用已验证的终态、取消和透明重试边界。
- [#36](https://github.com/Ricardo121380/cpa-rust-gateway/issues/36)：精确 Responses 续接保留完整历史。原因：连接内及持久续接经过共同历史重建和精确 lineage 路径。
