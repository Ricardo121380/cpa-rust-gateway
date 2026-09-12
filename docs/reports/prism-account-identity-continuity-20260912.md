# 账号身份、重复授权与运行状态修复

后续状态：用户已授权发布，`5b92e15` 已上线；见
[生产发布记录](prism-identity-continuity-production-20260912.md)。以下保留本地修复交付时的状态与证据。

2026-09-12。代码基线 `2d4e27e`；本报告随修复代码一起提交。本批未部署，生产仍为
`a243aab`、存储 schema 24。候选代码使用 schema 25、123 个管理操作，保留四文件 SPA。

## 人工反馈的结论

| 问题 | 已确认原因 | 本批结果 |
| --- | --- | --- |
| 部分 SSO 没有邮箱 | 旧导入仅解析凭据本身，未读取已登录会话的资料 | 新导入自动查询；已有账号提供“读取身份”；返回的资料加密保存。两条线上 Console 尚未取回邮箱，见下文 |
| Codex 同邮箱出现两次 | 两条持久凭据记录的邮箱、subject、ChatGPT account、access token、refresh token 和上游均相同 | 目录按身份显示一行，标明“2 份授权”；“管理授权”保留每条记录的详情、重新授权及启停入口。没有删除、停用或迁移生产绑定 |
| 运行状态出现编号和“Codex 桥接 上游” | 该子页仍使用旧资源名称层 | 改用服务端运行配置中的身份、实际渠道名和协议/主机；原始 ID 只在技术详情及精确操作中使用 |

目录的归组只作用于展示，按相同渠道下已观测邮箱或电话归组。用户名相同或身份缺失不自动归组。
运行状态仍按每条接口连接展示，因为每条绑定的认证、调度、并发和权益可能不同；同邮箱多条
运行连接有明确的协议/主机语义。Autoreg 只作为来源显示，不充当账户姓名。

## SSO 链路与线上限制

Web/Console 的身份端点依据 grok2api 的
[Session 身份实现](https://github.com/chenyme/grok2api/blob/main/backend/internal/infra/provider/sessionidentity/session.go)，
固定为 `https://grok.com/api/auth/session`。服务端使用凭据中的 Cookie、既有 Web 出口与
Chrome 传输，最多 4 个并发请求，整体 15 秒，响应最多 64 KiB，不重定向、不自动重试。
列表 GET 不触发 Provider 请求。资料只允许邮箱、电话、用户名；blocked/未认证响应中的残留
邮箱不接受，opaque user ID 不转换成姓名。Build 在缺少邮箱时继续从既有 issuer 获取资料。

新增 `POST /admin/native-accounts/{account_id}/identity`，受管理鉴权及 CSRF 保护，要求
当前 revision。导入成功但资料读取失败时保留有效凭据，界面明确区分这两个结果。403、会话
未认证和临时不可用分别返回安全错误；Provider 的认证失败不会被当作管理员会话失效。

本批在线诊断只查询现有两条 Console 的资料：各一次 GET 均返回 403；随后对其中一条再做
一次响应特征检查，确认出现 Cloudflare challenge。共 3 次资料 GET，0 次推理、0 次重试、
0 次生产状态修改；没有 Web SSO 记录可供本轮在线检查。既有服务没有配置 Web 专用代理。

这些在线检查使用标准 curl，不能当作新编译 Chrome 传输在 Oracle 上的验收。两条 Console
的真实邮箱仍未取回，也不能由 403 推断 SSO 已过期。后续需在可正常访问该官方资料端点的
会话/出口上验证候选传输；本批未改代理、未处理或绕过第三方验证、未重做授权。

## 存储与运行一致性

- schema 25 增加原生账号身份观察表，资料使用现有 AEAD 加密，并绑定账号与凭据密文指纹。
- 写入同时检查凭据 revision、密文指纹和先前观察指纹，防止凭据变更或同毫秒并发晚响应覆盖。
  资料变化更新 inventory generation；凭据轮换后旧观察不能继承，纯元数据变化不丢失资料。
- 不改原 SSO 凭据格式或认证状态。已有 Build compact v2 身份保持兼容。
- 普通运行身份来自实际 serving 配置，不能拿浏览器正在编辑的草稿给运行对象贴标签。
  原生账号可展示更新的身份元数据；运行状态及分页仍属于同一不可变观察快照，契约明确区分。
- SQLite 身份读取进入已有有界阻塞执行器；网络阶段不占用 Actix 工作线程。409 只重读列表，
  不自动重放身份读取写请求。

未来发布前需备份并验证 migration 25。回退到当前生产 `a243aab` 时应执行相应 schema 25
回退/恢复备份，不能只换旧二进制；该目标版本已支持 compact v2，本批没有新增凭据格式。

## 验证证据

以下为本批实际运行结果，没有以历史项目测试数量代替本次验证：

| 检查 | 本批结果 |
| --- | --- |
| 前端类型与单测 | 类型检查通过，269 项单测通过 |
| 账号相关 Chromium E2E | account-actions、account-presentation、provider-pools、native-accounts 共 16 项；最终受影响的前两组 9 项重跑通过 |
| Provider 身份回归 | 7 项，覆盖会话状态、资料字段、Build 缺邮箱补取、加密/CAS/凭据变化 |
| HTTP managed_resource_inventory | 11 项通过，最终改动后重跑；包含 SSO 导入/读取、409、拒绝响应、重启后加密资料读取 |
| 运行池与装配 | 11 项 adapter、1 项 native descriptor、3 项 HTTP inventory 通过 |
| 数据库迁移 | 7 项通过，包含新表与回退库存 |
| Rust 静态检查 | 相关五 crate 全 targets clippy 通过；最终 gateway all-targets 重跑通过 |
| 契约与嵌入 | sync-contract、`node scripts/check-management-spa.mjs` 通过：123 操作，四文件双构建 |
| 仓库边界 | source-policy、crate-boundaries 通过；本批提交前另检查文档链接、diff 和 staged secret scan |

EgoLite Chromium 152 使用实际本地 gateway、隔离合成状态进行操作，区别于上述 fixture E2E：

- 1440×900、1280×720、390×844；目录一条 Codex 身份、两份授权，详情动作指向正确凭据。
- 运行页 6 条真实网关返回的合成绑定，身份、渠道、协议/主机可见，无旧编号/“桥接”主标签。
- 手机同时显示认证、调度和停用；详情两侧各 12px、焦点在面板内、Escape 关闭，无横向溢出。
- 深色、增强对比、减少动态/透明度；两份授权的操作布局一致。

本地查看：<http://127.0.0.1:61307/admin-ui/#/accounts>。仅 loopback，使用临时合成状态，
EgoLite 验收页已保留。预览凭据只在本机私有临时目录，不写入报告。正式域名仍是上一版，
本批没有把合成账号导入生产，也没有执行新的生产发布。

证据：[脱敏诊断与浏览器记录](evidence/prism-account-identity-continuity-20260912.json)、
[目录](evidence/prism-account-identity-continuity-20260912/deduplicated-directory.png)、
[运行状态](evidence/prism-account-identity-continuity-20260912/runtime-1440.png)、
[手机深色](evidence/prism-account-identity-continuity-20260912/runtime-390-dark.png)、
[授权管理及辅助偏好](evidence/prism-account-identity-continuity-20260912/grants-390-accessible.png)。
截图均为合成资料；线上诊断只保留相等性/状态，不保留真实身份或凭据值。
