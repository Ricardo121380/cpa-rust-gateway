# Grok 首次授权与重新授权：真实验收通过

2026-09-11。验收使用用户指定的 Chrome、实际嵌入式 Prism、独立本地 gateway 和真实
Grok 官方 Device OAuth。此前缺少管理端接线的问题已由 `21afa41` 补齐。

| 项目 | 本次实际结果 |
|---|---|
| 首次授权 | 用户在 Chrome 官网完成授权；实际 token grant 加密写入原生账号池；账号页可见 |
| 重新授权 | 同一 Chrome 登录态、同一 Grok Build 权限范围重新授权成功；更新原账号 |
| 身份与记录 | 账号数量保持 1，账号 ID／导入身份不变，revision 从 0 增至 1 |
| 秘密与审计 | 密文发生变化、key version=1；新增 1 条 device_reauthorized 审计记录 |
| 重启持久化 | 独立 gateway 重启后，账号 ID、revision 和密文保持；实际管理 API 与 Chrome 页面重读成功 |
| 数据完整性 | SQLite quick_check 与 foreign_key_check 通过 |

[脱敏收据](evidence/prism-grok-auth-live-20260911.json)包含实际账号 ID、revision、数量及
重启核对结果；没有 token、邮箱、验证码或明文凭据。取消、过期、错误账号和旧 revision
拒绝另有合成回归，没有为了这些失败场景反复操作真实 Provider。

首次在侧边栏建立的旧会话按用户要求取消；Chrome 的扩展弹窗阻挡由用户解除后，重新
开始了以上两项成功授权。后续验收继续使用 Chrome，不使用侧边栏。

本次只验收账号授权与持久化，没有对该新账号发送推理请求；此账号尚无模型／路由绑定。
生产 CPAR 的部署、配置和存储记录未改写。该结果不代表 Codex／Claude／Kiro 的首次 OAuth、
全量批量导入或“保存并应用”的运行时切换已完成。

当前实际本地查看入口：`http://127.0.0.1:61700/admin-ui/#/accounts`，已在 Chrome 保留。
