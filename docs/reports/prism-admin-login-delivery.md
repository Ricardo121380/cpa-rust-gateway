# Prism 管理员登录交付

已上线：[Prism 管理端](https://cpar.142857142.xyz/admin-ui/#/unlock)。
部署源代码为 `b30d191cd61bc974dc29cc8e56433542fdb268b8`；本报告对应本轮实际运行结果。

## 界面与登录方式

登录页现在只保留 Prism 标识、管理员登录标题、账号、密码和登录按钮。
沿用 V4 Apple Liquid Glass：400px 卡片、银白/石墨灰、Apple 蓝、48px 表单控件、
手机外侧 12px 留白。输入框边界和占位文本补足对比度；必要的错误及首次改密提示按状态出现。
设置页展示管理员身份和会话到期时间，提供改密与退出，渲染/构建说明默认折叠。

设计由本会话完成，通过 OpenDesign MCP 在既有项目
`prism-gateway-console-redesign-a735` 保存、回读并检查 `prism-v4-admin-login.html`。
回读与仓库文件逐字节相同：SHA-256
`076f8fe029460c5ffbbc1545126e1ed9aa1259f1dfedb77f792404925603a46a`。
没有使用 OpenDesign 内置模型生成器或外部模型 CLI。

参考：[grok2api 登录页](https://github.com/chenyme/grok2api/blob/main/frontend/src/features/auth/login-page.tsx)、
[CPA Manager Plus 登录页](https://github.com/seakee/CPA-Manager-Plus/blob/main/apps/web/src/features/login/LoginPage.tsx)。
后者仍以 Admin Key 为常规入口；本次按用户要求实现真正的管理员账号密码登录。

## 管理员与会话

- 默认账号 `admin`；随机初始密码通过本机本人可读文件交付，文件权限 0600、父目录 0700，未写入聊天、日志、仓库或前端包。
- 首次登录必须改密。初始会话只有改密/退出权限，不能读取或修改管理资源。
- 使用 Argon2id v19、19 MiB、两轮、单 lane 和独立随机盐。密码原样验证，不裁切空格或 Unicode。
- 普通会话最长 8 小时，首次改密会话 10 分钟。最多 32 个会话；密码验证成功后可替换最旧会话，避免刷新遗留会话造成长时间登录阻塞。
- 每分钟最多 10 次密码验证、最多 2 个并行密码任务。校验和持久化不阻塞 HTTP worker。
- CSRF 自动生成和发送；密码变更撤销全部会话，退出撤销当前会话。前端仅在内存保存会话，刷新、过期和退出会清理缓存、轮询及版本上下文；旧响应不能恢复失效会话。
- 单管理员哈希保存在私有 `admin-auth.sqlite3`，独立于配置备份和控制数据库 schema 22。现有命令行管理方式继续兼容。

契约与操作说明：[CR](../change-requests/prism-admin-password-login.md)、
[管理员登录交接](../handoffs/prism-admin-password-login.md)、
[设计产物](../design/prism-v4-admin-login.html)。

## 本轮验收证据

| 范围 | 实际结果 |
|---|---|
| 前端类型/单测 | 类型检查通过；22 文件、256 个单测通过；最终会话引用调整后额外重跑 19 个 ownership 用例通过 |
| 前端 E2E | 全量 127 用例首次 126 通过；一项旧英文说明断言随精简文案更新，恢复必要的翻译范围提示后 3 个 i18n 用例重跑全过；全部 14 栏目入口、移动布局与既有功能已覆盖 |
| Rust 登录/契约/原安全边界 | 3 个登录集成、13 个 OpenAPI、4 个原安全边界测试通过 |
| Rust 会话/存储/装配 | 会话上限/到期/限流 2 个，存储权限/防覆盖/CAS 1 个，gateway binary 129 个测试通过；相关 Clippy 通过 |
| 构建与契约 | `sync-contract`、四文件嵌入、CSP、双构建一致性及 `check-management-spa.mjs` 通过；生成 111 个操作 |
| 本地真实 gateway | 私有初始化、错误密码/Origin/CSRF、首次受限、首次改密、真实草稿写读、撤销、重启持久化通过 |
| 真实嵌入浏览器 | Chromium 151.0.7922.34；1440×900、1280×720、390×844 的浅/深色登录和改密页；高对比、减少动态、键盘焦点、刷新清理通过，无 page error |
| 既有实际操作 | `prism-v4-local-acceptance.py --priced --browser-flow`：新账号登录下十阶段真实浏览器流程通过，包括模型交接、候选 CRUD、授权、校验、发布、审计、回滚、会话拒绝及移动辅助偏好；Provider 为 loopback 合成服务 |

源代码提交：`35a8af7`（实现）、`f9884a8`（精确声明 Argon2 依赖边界）、
`b30d191`（会话上限恢复行为）。首轮正式门禁在 Rust/Clippy 通过后发现依赖白名单漏项，
已修正；没有关闭或放宽其他依赖边界。后续门禁/签名/线上回执如下。

复现登录验收（合成配置，无真实 Provider 请求）：

```sh
cargo build -p gateway --bin gateway
python3 scripts/test-admin-login.py --binary target/debug/gateway --browser-output output/admin-login-browser --report output/admin-login.json
```

## 使用与保留范围

首次使用由本人输入初始密码，再设置自己的新密码。生产验收只验证初始会话限制和退出，
保留首次改密状态给用户。浏览器内存刷新清理仍是既有策略，不提供“记住登录”。
物理 iPhone/Safari/Firefox 不作为此次实际浏览器验收结论。

本次登录改动不处理原有历史计费修复队列。控制数据库、原活动配置和 87 条历史待修复
记录在部署前后核对。DNS、Caddy 路由及其他服务未改；没有主动调用真实 Provider。

## 正式发布与线上验收

- [最终完整门禁 34471891708](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34471891708)：Fast、完整供应链和 Required delivery gate 全部成功。
- [最终签名发布 34471888519](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34471888519)：ARM64、x86_64 均成功。已独立校验 Cosign 身份、manifest、SBOM 和 receipt。
- VPS 上使用最终 ARM64 二进制和隔离的临时状态重复实际 gateway 验收通过，再切换正式 CPAR 服务。
- 线上登录验证了正确账号、独立会话/CSRF、首次受限、错误 Origin/CSRF 拒绝、退出失效、原 CLI 读回及数据 listener 健康。
- Codex 内置浏览器实际访问 HTTPS 域名，确认简洁登录 UI，无 console warning/error；未代替用户修改正式管理员密码。

运行二进制 SHA-256：
`c3857016363c8af6d71fc0c2d3e5eb04d5e22bd63d16cede20a0c6a4c0043fbd`。
停止到恢复健康耗时 **1236 ms**。
控制数据库 schema 22、原活动配置、87 条历史待修复记录保持一致。
Caddy 配置、进程与 Autoreg 状态均与切换前相同；两个 gateway listener 继续只绑定 loopback。

四个线上资源的 SHA-256：

| 资源 | SHA-256 |
|---|---|
| `/admin-ui/` | `1121e8b70f28997fd2684e757f40e862b036e0318144d75834ef18418ea7bc19` |
| `/admin-ui/assets/main.js` | `c2bc3c11df689c344b75a1417ee00a07786d53c8480276f7b97cbf1c39651e17` |
| `/admin-ui/assets/vendor.js` | `6b6ea2186c7d05e7e13e63310ea22f96e1fa0e9733a81c6436747f373199ac8f` |
| `/admin-ui/assets/index.css` | `696e0e09ef83f65b04c241bc0540d085d802b4d6cfe9bd1553bb8454149fd6cd` |

回滚材料保存在 VPS 私有目录 `/var/backups/cpa-rust-gateway/prism-admin-login-20260910`。
回滚目标为 `c7cfd2c0d8771187775d2cd846cb49a8cf536046`，只回切二进制；保留独立管理员库和
控制 schema 22，不执行历史数据清理或数据库降级。初始管理员密码通过私人文件单独交付。

本机详细日志、截图及值已脱敏的操作回执位于 `output/prism-admin-login/`。
生产登录页已在内置浏览器打开，可直接使用原域名从其他设备访问。
