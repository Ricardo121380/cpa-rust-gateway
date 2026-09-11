# Grok 首次授权与重新授权验收：未通过

2026-09-11。用户要求验收首次授权与重新授权，沿用已指定的 Grok 渠道。
本次没有请求真实 OAuth grant、轮换生产 token、部署或改写生产账号。

## 入口与装配核对

| 必需路径 | 当前证据 | 判定 |
|---|---|---|
| 从无账号发起 Grok 授权 | 管理授权 start 先 require_credential；无新账号临时会话接口。Prism 添加入口只有 API Key | 未实现，无法验收通过 |
| 现有 Grok 重新授权 | serve 注入 CodexOAuthManagementWorkflow/OpenAiCodexOAuthExchange。Prism 仅 oauth_json 显示重新授权；Grok Build 凭据不属于此类型 | 未接入，无法验收通过 |
| Grok Device Grant 底层 | provider-grok/oauth.rs 已有 start、poll、interval、slow_down、denied、expired、Granted 处理 | 底层存在，不等于管理端接通 |
| 授权完成后入库及账号关联 | Grok 使用原生 GrokAccountPoolStore；管理端当前 Codex 回调调用 persist_oauth_credential_if_revision | Grok 管理会话尚无对应持久化／重新授权 CAS 路径 |

具体代码：`crates/gateway-http-actix/src/management_resources.rs` 的 start_credential_oauth、
complete_credential_oauth；`apps/gateway/src/deployment.rs` 的管理 workflow 装配；
`web/prism/src/features/accounts/AccountsPage.tsx` 的添加与重授权入口；
`crates/provider-grok/src/oauth.rs` 与 account_pool.rs 的已有底层能力。

## 本次实际运行

- `cargo test --locked -p provider-grok --test p6_01_build_oauth`：10 项通过，包含 Device Code
  状态机、scope、导入与秘密脱敏。全部是合成 transport，未与 auth.x.ai 交换真实授权。
- `cargo test --locked -p gateway-http-actix --test p10_04_management_resources oauth`：2 项通过。
  验证现有注入工作流与持久 Codex OAuth 状态恢复，不能证明 Grok 首次或再次授权。

## 完成前必须补齐

1. Grok 专用管理 Device OAuth 会话：无需预先制造含假 secret 的凭据；临时 device_code
   留在服务端；响应只有 verification URI、user code、过期与轮询状态；有界会话及 TTL。
2. 首次授权成功后通过原生 Grok store 加密落库，并让完整管理库存显示新账号及未绑定状态。
3. 重新授权绑定稳定原账号身份，校验账号一致性与 revision，保持关联，拒绝晚到旧 grant；
   取消／失败不得覆盖旧凭据。
4. Prism 对应新增／重新授权入口及错误恢复，然后在隔离环境完成真实 Grok 用户授权、
   持久化重读和单次受控调用。需要用户交互时应停在真正的 Grok 确认页面。

当前结果是验收未通过，不能用此前的 Device OAuth 历史记录、今天已有账号调用成功、
Codex 模拟授权或 token refresh 替代首次授权／重新授权验收。本报告没有将缺失实现标记完成。
