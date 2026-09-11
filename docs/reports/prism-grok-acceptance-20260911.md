# Grok 真实链路验收（2026-09-11）

用户指定使用 Grok 验收。本次复用既有 `cpar-grok-build` 客户端身份，通过 CPAR 公网
`/v1/responses` 执行 `grok-4.6` 普通响应与 SSE 各一次。每次限制 96 output tokens，
路由 max_attempts=1，不自动重试；未切换服务、发布配置、修改提供商凭据或清理历史。

## 本次结果

| 路径 | 实际请求 | 实际 Attempt | 持久事件与账本 |
|---|---:|---:|---|
| JSON | 1 | 1 | 响应完成、有文本、response ID 对应本机 Usage；唯一账本行 |
| SSE | 1 | 1 | created／文本增量／completed 均到达；唯一账本行 |

两次请求的 input/output/reasoning/cache_read/cache_creation/cached 六字段均与持久 Usage
逐字段一致。cache_read 和 cache_creation 保留 null；费用 confidence 均为 unpriced，金额
null，不能解释为零消费。没有本次请求的未解决物化失败。历史未解决数量前后均为 87。

[本次脱敏收据](evidence/prism-grok-acceptance-20260911.json)记录 request/response ID、
六类 token 值及当前进程 binary SHA256。该 hash 为
`e7f7f3abe81df0ab807efad2bbdcdccc26af9daf68b04b31eeeb0dac9cc12e22`，对应此前 V6 部署；
本地功能重构代码 `58b869a` 没有在本次上线。

## 验收边界与后续基准

这证明当前线上 Grok 的调用→事件→物化→账本链路可用，不代表新版首次授权、导入、
提供商向导、保存并应用或整个功能重构已经完成。上述能力完成后，以 Grok 作为真实接入
验收渠道；按实际平台能力选择既有账号／凭据方式，不把 Codex OAuth 流程冒充 Grok 授权。

本轮先验证既有身份与模型授权，再执行两次请求。客户端 Key 仅在内存和 SSH stdin 中传递，
没有输出或复制到服务器文件。生产数据库仅作只读关联核对，新请求由正常网关路径记录。
本机 `output/prism-grok-acceptance-20260911/` 保留禁止重放的预算预约与脱敏执行收据；
没有调用以前发布脚本的 before/after 模式或重置旧预算。
