# Grok Build reasoning 生产配置修复与真实复测

## 结果

2026-09-25 11:43:51 Asia/Shanghai，通过管理 API 草稿、校验、CAS 发布完成用户明确授权的配置修复。随后一次真实 Grok Build / grok-4.5 / Responses low 请求成功完成；服务端记录 succeeded、1 attempt、1 条账本。没有执行工具调用，也未把这次请求回放宣称为 OMP 桌面完整审批/写入验收。

## 发布范围

- 前驱：`production-aligned-20260915`，`rev-12`。
- 新活动配置：`production-grok-reasoning-20260925`，`rev-1`。
- 唯一业务字段变化：`p12-12-production-grok-build-candidate` 的 override 从 `{"allow_unlisted_model":true,"reasoning":false}` 改为 `{"allow_unlisted_model":true}`。
- 整张可见配置图比较通过；模型、端点、其他候选、别名、访问组、授权、Client Key、普通凭据元数据、连接与出口策略保持不变。持久差异另外包含两条凭据 ciphertext，这是 fork 为新版本 AAD 重新封装的设计行为，见 `management_mutation_service/configuration_edit.rs`；未导出明文或替换账号。
- Route 与配置 validation 均为 valid，error_codes 为空；发布绑定原 active 和生命周期事件 102，未自动重放写入。
- 当前运行进程 PID 1740127、二进制 SHA-256 `6f4eb0924987734b4ca56178fe3fde213cbd1e51a7d40dacb7053f869348b94a` 未变，仍为已签名 `3a8e5af`。本次是配置热发布，无重新部署/服务重启、DNS/Caddy/Autoreg 改动。
- `2ebb005` 修复后续创建脚本默认值；它不是本次需要重新部署的运行时二进制变更。

操作中先因错误使用只支持 POST 的凭据集合 URL 得到只读 404，随后改为权威契约的分页 GET `/admin/credentials`。草稿差异保护在识别 fork 密文重封装前暂停了一次；随后读取现有 draft rev-1 并按精确字段核对继续，未重建草稿。凭据列表投影键名修正也发生在发布前，只读确认后继续。上述过程没有发生未确认状态下的重复写入。

## 真实验证

2026-09-25 11:46:44–11:46:49 Asia/Shanghai，使用现有 `cpar-grok-build` Client Key 经公开 HTTPS 域名发送一次已捕获的合成 OMP 请求。保留相同 model、stream、low、summary:auto、encrypted-content include、2048 输出上限及六个工具定义；不降级思考，不换 Provider，不自动重试。Client Key 仅在进程内使用，原 CLI 配置字节未变。

| 检查 | 实际结果 |
|---|---|
| HTTP / 内容类型 | 200 / text/event-stream |
| SSE 终态 | response.completed；无 error/response.failed |
| 返回内容 | reasoning delta 与 1 个 function call；未执行 function call |
| 客户端墙钟耗时 | 5.073 秒 |
| 服务端请求耗时 | 2879 ms；不同于客户端网络总耗时 |
| 请求记录 | succeeded；error_code=null；1 attempt |
| 用量 | input 3123、output 121，其中 reasoning 38；cached 0；未观测 cache read/creation 保留 null |
| 计费物化 | 1 条账本；unpriced、金额 null，不能称为免费或零消费 |

请求 ID：`p1-request-b763edcc195a3e02ae4eb7b837c728cb-0`。本次是新授权的 1 次 CPAR 请求回放，不更改或重置 OMP 原阶段任务与重试累计额度。

## 协议影响与回滚

Responses Route Explain 选中目标候选。恢复 Reasoning 后，Chat Completions 对该候选返回 excluded / protocol_transform_unavailable，这是现有私有 reasoning 安全门禁的预期行为；不宣称该路由同时兼容 Chat。其他提供商候选未变。

前驱配置保留，可通过管理端正常 rollback 流程、当前 revision/active/lifecycle CAS 回退。回退前再次核对当前活动版本和并发变更；不要恢复历史数据库或轮换凭据。本次未实际执行生产回滚。仅本人可读的配置计划、前图和发布回执位于服务器 `/var/tmp/cpar-grok-reasoning-20260925`。

## 证据与未覆盖项

- [配置发布、校验、差异与进程](evidence/grok-reasoning-production-20260925/publication.json)
- [一次真实 SSE 回放](evidence/grok-reasoning-production-20260925/canary.json)
- [持久请求与账本投影](evidence/grok-reasoning-production-20260925/request.json)

本次已解决并实测原 Responses low 的配置阻断。OMP 原会话内的 GUI 发起、审批、受管文件写入、撤权与取消仍需在 OMP 的独立验收范围内继续；此处没有修改 OMP 代码或任务预算，也不宣布其 M2 完成。
