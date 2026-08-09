# Codex/ChatGPT OAuth 与凭证兼容性

状态：`BACKEND_PASS_WITH_STAGING_BOUNDARY`（导入、直接 OAuth 管理接口、refresh、CPA/sub2api
导出和 metadata 已实现并通过本地回归；官方 runtime 的隔离 staging 12/12 证据仍有效）。正式
管理 UI、后台自动刷新和跨重启的 pending OAuth 会话不在本次后端收口范围内。

CPAR 将 Codex/ChatGPT 凭证统一投影为
`OpenAiCompatibleRuntimeCredential::CodexOAuth`，接受以下来源：

1. CPAR 原生 `kind: "codex_oauth"` 文档；
2. CPA flat JSON（包括 `access_token`、`refresh_token`、`account_id`、`expired`/`expires_at`）；
3. sub2api 的 OAuth JSON（`credentials` 或 `token` 嵌套对象，以及 `type: "codex"` 形状）；
4. ChatGPT Go `auth_mode: "chatgpt"` + `tokens` + `_meta` envelope。

导入器只从认证字段 allow-list 生成 runtime secret，拒绝未知顶层认证字段、重复 JSON 名称、空
token、超大文件和无效过期值。过期时间按显式毫秒/秒、`expires_in` 或受限 JWT `exp` 推导；
刷新响应可以省略 `refresh_token` 或 `id_token`，此时保留当前值，符合 CPA/Sub2API 的 rotation
行为。账号绑定漂移会在 CAS 写入前拒绝，避免把一个账号的刷新结果写入另一个账号。

## 管理 metadata

套餐、额度、平台、邮箱和来源格式不是认证 secret，已进入独立的受保护 metadata read model，
供管理前端后续展示：

- `plan` 接受 `plan_type`、`plan`、`package`、`package_name`、`subscription_tier`；
- `quota` 接受有界标量，或 `_meta/extra` 中 `used_percent`、`reset_after_secs`、`balance`
  的有界摘要；
- `platform`、`email`、`source_format` 均按有界字符串投影；邮箱只在受保护管理接口返回，
  Debug/log 使用 `[REDACTED]`；
- 未知或疑似 secret 的 metadata 不会被提升为认证字段，也不会写入日志、receipt 或公共响应。

这样既保留用户查询套餐/额度/平台/邮箱的需要，也避免把 SSO、Cookie、opaque provider bookkeeping
误当作可发送的凭证字段。

## 导出

管理端显式选择 `cpa` 或 `sub2api`：

- `cpa` 输出 incumbent flat `type: "codex"` envelope；
- `sub2api` 输出 nested OAuth `credentials` 及受限 `extra` metadata。

两种输出都在内存中短暂生成，响应 `Cache-Control: no-store`，不进入普通 GET、审计正文、日志或
receipt；导出再经 CPAR importer round-trip 后，Bearer、account binding 和 expiry 语义保持一致。

## 直接 OAuth 生命周期

`start` 生成一次性 state 和 S256 PKCE challenge，使用固定官方 client/loopback redirect、
OpenID/profile/email/offline_access 以及 connectors scope，并返回一次授权 URL。重复 start 在
同一 pending session 上幂等，避免覆盖浏览器已打开的 state。浏览器回调可由 loopback relay
转发为 `{state, code}`，也可提交完整 loopback URL；provider `error`、冲突 state、过期 state 和
重复 callback 都是有界、无值的分类错误。

token exchange/refresh 均固定官方 token endpoint；refresh body 采用 CPA/Sub2API 的
`grant_type=refresh_token` + `client_id` + `scope=openid profile email` 形状。刷新在单 credential
范围内有进程内互斥 claim，并在持久层用 Config Version + Credential revision CAS，失败不会覆盖
新 refresh token。

OAuth HTTP 默认直连并禁用环境代理；需要时只能通过 serve 的显式、校验过的本地 DNS SOCKS5
`--codex-oauth-proxy` 指定独立出口。生产部署仍未修改。

## 验证

```text
cargo test --locked -p provider-openai-compatible -p gateway-http-actix -p gateway --all-targets
# gateway 85 + component smoke 1；gateway-http-actix lib 54 + 管理资源 3；provider 35
cargo clippy --locked -p provider-openai-compatible -p gateway-http-actix -p gateway \
  --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
python3 -m py_compile scripts/p12-codex-oauth-relay.py
```

Oracle Singapore ARM64 隔离 staging 的既有真实 CPAR receipt
`e2e-codex-bridge-v2-current-source-final.json` 记录 Chat/Responses/Messages × JSON/SSE ×
text/tool 共 `12/12`，`network_send_count=12`、`value_free=true`。最近一次 fresh harness 使用的
client-key artifact 已过期，首个请求在路由前返回 401，因此不计作 OAuth 失败或新的通过；此前
有效 key 的 12/12 证据仍保留。生产 listener、Caddy、DNS、旧 CPA 和正式 CPAR 图均未修改。
