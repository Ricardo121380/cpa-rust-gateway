# P12-10I-03 Grok SSO→Build OAuth 与 Console egress 受控回执

| 字段 | 值 |
|---|---|
| 日期 | 2026-08-05 |
| 环境 | Jakarta 源侧受控内存执行；Oracle Singapore 只读网络分类；未启动 staging 图 |
| 变更边界 | `CR-P12-10I-004`；最多 5 个新 SSO，固定串行、每个只尝试一次 |
| Secret 处理 | 未写入本机、receipt、日志或 Git；未输出账号、SSO、OAuth、endpoint、model、请求/响应正文 |

## SSO→Build OAuth

| 项目 | 结果 |
|---|---|
| SSO 候选尝试 | `5`，串行、无重试、无跨账号 fallback |
| 产生新的 Build access/refresh | `0/5` |
| 候选 1–2 | 源 helper 返回通用 `Exception`；原始正文/状态未保留，不能细分 |
| 候选 3–5 | Accounts 检查为 4xx（helper 按设计继续）；Discovery 为 2xx；Device Code 为 2xx；Verification redirect 与 Device Verify 均为 4xx；固定终态 `device_verify_rejected` |
| CPAR Build 导入/请求 | `0`；没有新 OAuth，因此没有 staging Build 图、JSON 或 SSE 请求 |

这证明当前抽取的 5 个 SSO 在本次 Device Verify 边界不可用于换取新的 Build OAuth；不证明全部 SSO 池永久失效，也不证明 CPAR Build adapter 有代码错误。

## Console egress 只读分类

从 Oracle Singapore 发出一次无凭据、无请求体、无重定向跟随的只读网络探测，仅保留固定类别：

| 项目 | 结果 |
|---|---|
| TLS 验证 | `ok` |
| DNS/连接 | 已建立（仅记录地址存在，不记录地址） |
| HTTP | `4xx` 类别 |
| 重定向 | `0` |
| Console 认证推理 | 未发送；没有重放 P12-10I-02 tuple |

TLS 和基本连接正常，但未携带 SSO 的请求收到 4xx；结合 P12-10H 中 25 个不同 Console 凭据统一 `EgressRejected/egress` 的证据，当前更符合 Provider 侧访问/WAF/egress 准入问题，而不是 CPAR `/v1/models` 或账号导入问题。精确 HTTP 状态和上游正文未保留，不能进一步猜测。

## 不变性

- 未写入 CPAR staging 数据库、Config Version、route、Client Key 或 Credential pool。
- 未重启或改变生产 CPAR、旧 CPA、grok2api、CC Switch、Caddy、DNS 或公开流量。
- 未重放 P12-10I-02 的失败 refresh tuple，也未发送新的 Console 推理或 SSE。

## 结论

`P12-10I-03` **BLOCKED_WITH_EVIDENCE**：SSO→Build OAuth 在 5/5 Device Verify 边界失败；Console 的无凭据 egress 分类显示 TLS/连接正常但 Provider 返回 4xx。Build CPAR JSON/SSE 及新的 Console CPAR 推理没有被虚报为通过。
