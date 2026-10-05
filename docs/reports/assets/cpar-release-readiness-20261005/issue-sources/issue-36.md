# #36 CPAR-27：精确 Responses 续接保留完整历史

State: OPEN; Updated: 2026-09-29T13:31:32Z

## Parent

[实施规格 #9：CPAR 现有渠道可靠性与管理体验收敛](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9)

## What to build

previous_response_id 续接保留文本、工具与已支持语义，并只使用原成功请求的精确账号和配置 lineage。

实施编号 CPAR-27；实施批次：B。覆盖用户故事 57, 64, 72, 73；验收场景 T09, T11。

## Acceptance criteria

- [ ] 公共端点续接以已保存请求和成功输出构造有界历史，保留工具 ID／结果、思考及可表示签名／引用。
- [ ] Responses 编码再解码的历史路径不得悄悄丢失新增语义；无法等价时明确拒绝，不用明文合成私有推理状态。
- [ ] 固定实际成功的配置、Provider、渠道、候选、账号及凭据 revision，并重新检查资格；至多执行一次，无 sibling、出口或 Provider fallback。
- [ ] 异主、过期、已删除、模型／revision 变化和原账号不可用分别安全失败；不得转发下游 response ID 绕过本地所有权。
- [ ] 公共非流式／SSE 及重启后的正反例通过，上游调用计数证明失败不换号或重试。
- [ ] 记录受影响行为的高层本地验证、必要的 EgoLite 产品验收与审查证据；静态存在、fixture、真实渠道和生产结论分开，结果使用 PASS／FAIL／NOT_RUN／BLOCKED。
- [ ] 保留账号／渠道／权限及数据边界；涉及权威管理契约时单向同步生成客户端，遵守既有前后端归属与跨边界记录；必要 ADR 与相关陈旧测试入口随本切片修正。
- [ ] 在实际 checkout 上复核范围，完成适用 Task／Phase 门槛；本票不自动授权真实账号、收费请求、服务变更或部署，具体外部操作以已明确目标和范围为准。

## Blocked by

- [#28](https://github.com/Ricardo121380/cpa-rust-gateway/issues/28)：三协议非流式工具多轮闭环。原因：历史重放需要完整工具关联。
- [#30](https://github.com/Ricardo121380/cpa-rust-gateway/issues/30)：思考语义与 Chat 准入一致。原因：需要已明确的思考表示。
- [#31](https://github.com/Ricardo121380/cpa-rust-gateway/issues/31)：签名、引用与缓存语义保真。原因：需要能保留签名、引用和缓存信息的表示。
- [#35](https://github.com/Ricardo121380/cpa-rust-gateway/issues/35)：已保存 Responses 的所有权与重启读取。原因：依赖可信的完成记录及精确所有权读取。
