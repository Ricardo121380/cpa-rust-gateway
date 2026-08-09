# P12 Codex runtime and OAuth management final review

日期：2026-08-09

## Verdict

`PASS_WITH_BOUNDARY`：官方 ChatGPT/Codex runtime 的隔离 Oracle Singapore ARM64 staging
证据保持 `12/12 PASS`；OAuth 管理后端的导入、直接 PKCE callback/exchange、refresh CAS、
CPA/Sub2API 导出、metadata 和错误分类已通过本地回归。正式管理 UI、后台自动刷新及跨重启
pending session 尚未承诺为本阶段完成。

## 修复内容

1. 不再把 OAuth 凭证当成 Krill/API-key：导入 CPA flat、Sub2API nested、ChatGPT Go envelope，
   保留 account binding、expiry、refresh/id token 和受支持 metadata。
2. 官方 runtime 使用固定 Codex endpoint、账号绑定头、Codex identity headers、`store:false`
   和 upstream SSE；可见文本/tool 生命周期保持三协议投影，private encrypted reasoning 不泄露。
3. 管理 OAuth start 使用一次性 state/S256 PKCE 和固定 loopback redirect；同一 pending start
   幂等，错 state 不会 DoS 合法会话，provider rejection 有明确安全分类。
4. refresh 采用 CPA/Sub2API form、可选 rotation 字段保留和双层 CAS；并发调用由 single-flight
   claim 收口。
5. status 在重启后根据 active encrypted `oauth_json` 做 durable `complete` fallback，避免
   “已经授权但页面失败”的假阴性。

## Review gates

```text
cargo fmt --all -- --check                                      PASS
cargo test --locked -p provider-openai-compatible \
  -p gateway-http-actix -p gateway --all-targets                    PASS
  provider 35; gateway-http-actix lib 54 + management resources 3;
  gateway 85 + component smoke 1
cargo clippy --locked -p provider-openai-compatible \
  -p gateway-http-actix -p gateway --all-targets --all-features \
  -- -D warnings                                               PASS
python3 -m py_compile scripts/p12-codex-oauth-relay.py          PASS
```

## Staging evidence

当前源码的有效 ARM64 staging receipt
`e2e-codex-bridge-v2-current-source-final.json` 记录固定/尝试/成功 `12/12/12`，覆盖
Chat、Responses、Messages 的 JSON/SSE text/tool；`value_free=true`、`network_send_count=12`。
一次使用过期 client-key 的 fresh harness 在路由前收到 401，没有产生 upstream attempt，故不计为
OAuth 失败或新通过。管理 status 已复核为 `complete`，且 control-plane audit 只读核对到一次
`credential_oauth_rotated`（不含 secret）。生产 CPAR、旧 CPA、Caddy/DNS 和正式
凭证池均未触碰，也没有启动 GitHub CI。

## 明确剩余边界

- 浏览器回调必须在操作员机器上使用 loopback relay（或粘贴完整 loopback callback）；远端
  CPAR 不直接伪造公网 redirect。
- pending state/verifier 仍是 process-local，staging 在授权过程中不可重启；已持久化的 active
  OAuth credential 重启后可正确显示 `complete`。
- 后台定时 refresh、正式管理 UI、生产切换和新的真实 HTTP 6/6 fresh matrix 需要后续显式
  任务/有效 client key，不能把旧 receipt 或 stale-key 401 夸大为新证据。
