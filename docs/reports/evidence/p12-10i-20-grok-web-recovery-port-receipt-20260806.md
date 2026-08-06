# P12-10I-20 Grok Web recovery port receipt

日期：2026-08-06

## 范围

本切片把 grok2api Web 请求的浏览器请求轮廓和受控恢复边界移植到 CPAR：

- Web outbound request 增加 Chromium Client Hints：`sec-ch-ua`、`sec-ch-ua-mobile`、`sec-ch-ua-platform`、`sec-ch-ua-arch`、`sec-ch-ua-bitness`；
- 403 的第一次重试仍先失效当前 Statsig 签名；
- 新增账号绑定的 `GrokWebEgressRefresher` 注入点，可在下一次尝试前返回带新 Cookie/已验证代理的全新 session；
- refresher 是 adapter-local、每请求至多一次的显式 recovery，不修改全局代理、全局 Cookie 或其他账号。

## 验证

- `cargo fmt --all`：PASS
- `git diff --check`：PASS
- `cargo test --locked -p provider-grok --test p12_10e_console_web_runtime --test p12_10i_web_inference_runtime`：13/13 PASS
- 回归断言确认 403 后 refresher 恰好调用一次，并保持同一 `egress_session_id`。

本切片未启动 FlareSolverr、未轮换真实代理、未发起 Grok 上游请求，也未修改服务器、生产图、CC Switch 或 grok2api。真实 clearance/egress provider 仍需后续显式配置和隔离 E2E 才能宣称可用。
