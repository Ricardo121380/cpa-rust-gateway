# P12 Codex 公网 SSE framing 排查

日期：2026-08-09

## 结论先行

本轮没有发现 CPAR SSE 解码器或 Caddy `reverse_proxy` 会主动改帧/整段缓冲的证据，
因此没有提交“伪造终帧”“放宽 EOF 成功条件”或强制修改 Caddy 的代码/配置。

公网当前真正的阻断点是凭据状态：使用已保存的真实测试 Client Key 时，生产
`/v1/models` 仍能返回 200，但 loopback 数据面与公网数据面在开始推理前都返回
同一类 `CredentialUnavailable` 503，响应没有 SSE 字节，因而不能把这轮结果称为
公网 framing 失败。

## CPA/Sub2API 对照

对照了官方实现的 Codex Responses 路径：

- CLIProxyAPI 的 Codex stream executor 只有在收到 `response.completed` 或
  `response.incomplete` 时才结束成功；Scanner EOF 且没有终止事件会产生
  `stream disconnected before completion` 类错误，不会补造成功帧。
- Sub2API 的 Codex identity 收口要求 `User-Agent`、`originator`、`version` 同源配对，
  版本不能低于上游门槛；其 SSE parser 也把 EOF 无终止事件视为失败，不把截断流变成成功。
- CPAR 已保持相同的 fail-closed 语义，并对官方 OAuth 的明确根级
  `max_output_tokens` 拒绝只做一次 request-local 删除重试；不会在上游断开后重放已输出内容。

来源：

- [CLIProxyAPI Codex executor](https://github.com/router-for-me/CLIProxyAPI)
- [Sub2API Codex identity/SSE implementation](https://github.com/Wei-Shaw/sub2api)

## 受控 A/B 证据（Oracle Singapore 隔离 staging）

使用同一份当前 staging revision 导出的 CPA 形状凭据、同一 upstream、同一文本请求，
仅改变 Codex identity 三元组：

| 身份轮廓 | 请求数 | HTTP 200 | `response.completed` | failure/incomplete |
|---|---:|---:|---:|---:|
| CPAR 既有 `codex_cli_rs` 0.144.1 | 2 | 2 | 2 | 0 |
| CPA/Sub2API 当前 TUI 0.146.0 | 2 | 2 | 2 | 0 |
| Linux `codex_cli_rs` 0.146.0 | 2 | 2 | 2 | 0 |

三组均完整收到上游 SSE 终止事件；因此没有证据支持“把 CPAR 版本头升级到 0.146.0”
能修复本轮问题。此前出现的 400 是上游明确拒绝根级 `max_output_tokens`，CPAR 已有
与对照实现一致的一次窄重试；删除该字段后六次直连均通过。

## Caddy/CPAR 组合验证（隔离，不改生产）

- 合成 SSE origin 经同版本 Caddy 默认 `reverse_proxy`：帧间约 350ms 的发送间隔仍被
  客户端逐步观察到，终止事件和 `[DONE]` 均保留。
- 同一 origin 显式 `flush_interval -1` 的结果与默认配置一致。
- 真实 CPAR staging `18192` 经 Caddy 临时 loopback 代理：默认配置 2/2、显式 flush
  2/2，全部 HTTP 200、`text/event-stream`、`response.completed`，无
  `response.failed`/`response.incomplete`，临时代理进程已清理。

因此当前没有足够证据修改 `/etc/caddy/Caddyfile`；生产 Caddyfile、DNS、旧 CPA、
生产 CPAR active version 均未改动。

## 当前边界与下一步

本轮公网复测被生产凭据状态挡住，而不是被 SSE framing 挡住：

1. 先在受控管理边界刷新或替换 active Codex OAuth（这会改变生产凭据 revision，需
   operator 单独确认）；
2. 刷新后仅用短矩阵重新走真实公网 Base URL，记录首个 `response.completed`/EOF 类别；
3. 只有出现“有 SSE 字节但终帧在公网丢失”且 loopback 同形通过，才继续查客户端出口、
   MTU/连接复用或边缘网络；在此之前不改 decoder、不注入终帧。

本报告不构成公网验收通过，也不触发 GitHub CI 或生产发布 closeout。
