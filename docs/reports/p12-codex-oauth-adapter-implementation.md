# P12 ChatGPT Codex OAuth adapter implementation

状态：`BACKEND_IMPLEMENTED; STAGING_RUNTIME_RECEIPT_VALID`；生产仍未切换。

## 根因与一次性修复范围

早期“授权成功但页面显示失败”由多条边界叠加造成，而不是简单账号失效：

1. OAuth 成功后的凭证已写入加密 control plane，但 status 只看进程内存 session；重启后找不到
   session 就误投影 `failed`；现在 active `oauth_json` 的 durable 状态投影为 `complete`。
2. 重复点击 start 会覆盖旧 state，浏览器持有的第一个回调因此失效；现在 pending start 幂等。
3. 错 state 原先可能终止合法 session；现在只返回无值 conflict，不消耗 pending session。
4. provider `error/access_denied`、token exchange 失败、持久化失败和 session 过期分别投影安全
   `failure_class`，便于排查而不泄露上游正文。
5. refresh token 可能轮换；现在 refresh body/response 对齐 CPA/Sub2API，省略的新 refresh/id
   token 保留旧值，且 credential 粒度 single-flight + Config Version/Credential revision CAS
   防止并发覆盖。
6. session 进入 complete/failed/cancelled/expired 后立即 zeroize 并释放原始 state/verifier；终态
   只保留摘要和 value-free 状态，不能再次取回 replay material。

## 按 CPA/Sub2API 移植到 Rust 的行为

- 兼容 CPAR 原生、CPA flat、Sub2API `token`/`credentials` 以及 ChatGPT Go
  `auth_mode/tokens/_meta` envelope；完整 envelope 进入 AEAD runtime；
- 直接 OAuth 使用固定官方 authorize/token endpoint、loopback redirect、S256 PKCE、OpenID/
  profile/email/offline_access/connectors scope、Codex originator 和受限 UA；回调由本机
  loopback relay 转为 management `{state, code}`，也可提交完整 callback URL；
- refresh 采用 `grant_type=refresh_token`、固定 client id、`scope=openid profile email`，不
  强制 redirect URI/offline_access，和参考实现一致；
- metadata 投影 plan/package、quota 摘要、platform、email、source format；邮箱和未知
  opaque 字段不进入日志/receipt；CPA 与 Sub2API export 可 round-trip 回 importer；
- OAuth HTTP 默认禁用环境代理，只允许显式、校验过的 SOCKS5 proxy，避免 Clash/TUN 或服务器
  ambient proxy 悄悄改变认证出口。

## 本地证据

```text
cargo test --locked -p provider-openai-compatible -p gateway-http-actix -p gateway --all-targets
gateway: 85 + component smoke 1
gateway-http-actix: lib 54 + p10_04 management resources 3
provider-openai-compatible: 35
cargo clippy --locked -p provider-openai-compatible -p gateway-http-actix -p gateway \
  --all-targets --all-features -- -D warnings       PASS
cargo fmt --all -- --check                           PASS
python3 -m py_compile scripts/p12-codex-oauth-relay.py PASS
```

## 隔离 staging 证据与边界

Oracle Singapore ARM64 隔离 staging 的有效 receipt
`e2e-codex-bridge-v2-current-source-final.json` 记录 Chat/Responses/Messages × JSON/SSE ×
text/tool `12/12`，`network_send_count=12`，无首败停止且 value-free。最近一次试跑使用了过期
client-key artifact，在路由前 401，未计为 OAuth 失败，也未作为新的通过数；不能用它替代既有
12/12 receipt。当前管理 status 对持久 active OAuth credential 返回 `complete`，control-plane
audit 中存在一次 value-free `credential_oauth_rotated` 记录。

正式管理 UI、后台自动刷新、加密 pending session 跨重启恢复，以及生产 CPAR 切换仍是后续边界。
生产 listener、旧 CPA、Caddy/DNS、CC Switch 和正式 credential pool 均未修改。
