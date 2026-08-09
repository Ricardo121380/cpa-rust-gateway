# P12 Codex 管理 API 契约（Backend-first）

状态：`IMPLEMENTED_BACKEND`
范围：`CR-P12-CODEX-MGMT-001/002/003`
日期：2026-08-09

本契约只收口后端管理接口和安全边界。正式管理 UI 延后；验收使用接口回归、最小 relay 和
隔离 staging。所有路由都在受保护的 management listener 上，按 `credential_id` 绑定资源。

## OAuth 生命周期

```text
POST /admin/credentials/{credential_id}/oauth/start
GET  /admin/credentials/{credential_id}/oauth/status
POST /admin/credentials/{credential_id}/oauth/cancel
POST /admin/credentials/{credential_id}/oauth/callback
POST /admin/credentials/{credential_id}/oauth/refresh
```

`start` 不接受任意 endpoint、token 或回调地址。服务端生成一次性 state、PKCE verifier 和
有界 TTL（当前 10 分钟）；持久化层只保存 state/verifier 摘要，原始值只在受控进程内存中
短暂存在。响应字段为：

```json
{
  "credential_id": "…",
  "state": "pending|complete|cancelled|expired|failed",
  "expires_at_ms": 0,
  "authorization_url": "only-on-start",
  "failure_class": "optional-safe-category"
}
```

同一个 credential 的 pending `start` 幂等并返回同一授权 URL，避免第二个浏览器窗口覆盖
合法 state。`callback` 接受有界 `state` + `code`，也接受 `callback_url`（query/fragment）；
provider `error` 会被分类为 `provider_rejected`。错 state 不会消耗 pending session，正确 state
只能成功一次。exchange 成功且 AEAD + Config Version/Credential revision CAS 持久化完成后才投影
`complete`；失败只返回 value-free 409/分类。

状态查询在进程重启后有一个明确的 durable fallback：如果 active `oauth_json` 已持久化而进程
内存中没有 pending session，返回 `complete`。pending challenge 本身仍是 process-local（CPA 和
Sub2API 的参考实现也采用内存 session），因此授权过程中不要重启 staging；需要跨进程恢复时
后续再增加加密 pending-session store。

`refresh` 在 credential 粒度做进程内单飞 claim，覆盖解密、网络请求和 CAS 写入；第二个并发
调用收到 `oauth_refresh_in_progress`，不会重复消耗旋转 refresh token。OAuth HTTP 默认不继承
环境代理；可用 serve 的显式 `--codex-oauth-proxy socks5://…` 指定独立出口。

不得返回 authorization code、PKCE verifier、access token、refresh token、Cookie、SSO、完整邮箱
或上游正文。

## 凭证导出

```text
POST /admin/credentials/{credential_id}/export
Content-Type: application/json

{"format":"cpa|sub2api"}
```

导出需要 management 权限、revision 校验和审计；只对已解密的短生命周期值执行格式转换，响应
设置 `Cache-Control: no-store`。`cpa` 是 flat incumbent envelope，`sub2api` 是 nested OAuth
envelope 并携带受限 `extra` metadata。secret 不进入普通 GET、错误、receipt 或日志。

## Metadata read model

```text
GET /admin/credentials/{credential_id}/metadata
```

允许返回套餐、额度、平台、邮箱、来源格式、credential kind/revision 和 secret-present 状态；
这些字段与认证 secret 分离。邮箱只有该受保护管理接口可见，Debug/log 脱敏；unknown/opaque
metadata 不会被当作认证字段或写入公共输出。

## 验收与变更边界

本地已通过状态迁移、PKCE/callback parser、token exchange/refresh CAS、导出 round-trip、
metadata 脱敏/权限和 OpenAPI/client generation 回归；隔离 staging 的既有 runtime 12/12 receipt
仍有效。正式 UI、后台自动 refresh、跨重启 pending session 和生产切换另行安排。

生产 CPAR、旧 CPA、Caddy/DNS、CC Switch 和正式凭证池在本阶段保持不变。
