# P12-10I-05 本机 Grok CLI 活跃会话探针回执

| 字段 | 值 |
|---|---|
| 日期 | 2026-08-05 |
| 变更边界 | `CR-P12-10I-006`；既有本机 Grok CLI 活跃会话、单轮最小请求、无工具、无重试 |
| 本机来源 | `~/.grok` CLI 状态；只检查文件存在性/形状、进程存活和固定结果类别 |
| CPAR/生产 | 未启动 CPAR staging，未读取/导出 CLI 凭证，服务器、生产图、旧 CPA、grok2api、CC Switch、Caddy/DNS 与公开流量未改变 |

## 只读前置检查

| 项目 | 结果 |
|---|---|
| CLI 活跃会话记录 | `1` 条；记录对应进程仍存活 |
| 本机 `auth.json` | 不存在；未发现可供 CPAR 直接导入的本地 Build OAuth 文件 |
| 敏感环境变量 | 未发现 Grok/XAI/OAuth/Token/Key 注入；未输出环境值 |
| `models` 命令 | CLI 自身退出 `0`；该命令只证明 CLI 命令可运行，不证明当前会话凭证可被 headless 调用复用 |
| 活跃会话与当前仓库目录 | 不匹配；未修改工作目录或会话记录 |

## 单次探针结果

| 项目 | 结果 |
|---|---|
| 请求方式 | 复用现有 CLI 的 `--continue` 单轮模式；固定最小提示，禁止工具/浏览/子代理 |
| 尝试次数 | `1`；无 retry、无 fallback |
| 退出状态 | `1` |
| 超时 | `no`；约 `612 ms` 返回 |
| 固定结果类别 | `auth_required_or_denied` |
| 输出形状 | JSON object；仅记录形状和字节数，不保留正文 |
| CPAR Build/Console HTTP | `0` |

## 结论

`P12-10I-05` **BLOCKED_WITH_EVIDENCE**。本机仍有一个运行中的 Grok CLI 进程，但当前仓库目录下的 headless continuation 无法访问可用认证，且本机没有 `auth.json`。因此不能证明该进程的内存会话可被 CPAR 使用，也不能把 CLI 会话 token 推断为 Build OAuth refresh token；本次未导出、转换或落盘任何凭证。

若要继续这条路线，需要在该活跃 CLI 会话实际所在的工作目录/交互上下文中完成一次明确的 CLI 请求，或由用户重新完成一次登录以生成受保护的 CLI 认证缓存；之后仍需另行确认其是否为 CPAR Build adapter 接受的 OAuth 形状。当前证据不足以进入 CPAR staging 或解除 Grok live gate。

## 不变性

- 未启动 OAuth 浏览器/Device Code，不打开第二个授权窗口。
- 未读取、打印、复制或持久化 Bearer、OAuth、SSO、账号身份、Endpoint、模型或请求/响应正文。
- 未修改 CPAR 源码、配置版本、数据库、监听器、服务器、公开入口或 grok2api 状态。
