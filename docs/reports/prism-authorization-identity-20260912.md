# 授权身份自动获取修复（2026-09-12）

后续：用户指定 EgoLite 后，已完成真实本地网关的账号显示、分组、连接详情及授权入口
浏览器复验，并修复 Codex 弹窗残留内部 ID。见 [EgoLite 验收报告](prism-egolite-20260912.md)。
下面的 Chrome 连接限制保留为当时事实，不再代表尚无实际浏览器验证。

用户明确：身份应在授权时获取，不采用人工补录方案。已据此继续修复，未部署生产。

## 已修复

- Grok Build 原实现虽然请求 openid/profile/email，却主动丢弃 id_token；现在授权与导入
  解析时提取邮箱、电话、用户名，只保留允许的显示字段，不把 token 或 subject 当名字。
- Device Grant 保存的是紧凑二进制凭据；上一轮列表仅解 JSON，读不到授权身份。现在
  原生列表会调用实际 Provider 解码器，支持旧 v1 和携带加密身份的 v2。
- 授权/导入没有可用身份时，以现有 OAuth grant 向固定 issuer userinfo 查询一次；
  10 秒、64 KiB 上限、禁重定向，返回 sub 必须匹配 access token 的 sub。没有增加 scopes。
  错误不覆盖已有身份，不丢弃合法授权；授权完成页明确区分未返回与暂时未取得身份。
- token 刷新省略身份时保留之前的字段；旧凭据仍缺身份时，运行时续期也自动查询 userinfo。重新授权在既有同账号/CAS 检查通过后保留缺省字段。
- 授权入口不要求手工名称；导入标记、Autoreg 来源、随机 ID 均不再成为身份显示的兜底。

userinfo 地址和现有 scope 由
[官方 OIDC 配置](https://auth.x.ai/.well-known/openid-configuration)核对。
这些信息仅用于显示，不代替鉴权或账户权益证据。

## 当前验证

- Provider 身份新回归 4 项：id_token→加密载荷→重读→刷新保留、v1 兼容和 userinfo 获取、
  错误 subject / 获取失败 / 刷新账号变化拒绝，以及旧凭据续期自动取得身份。
- 既有 Grok OAuth / 续期回归分别 10 / 8 项通过。
- Device 授权回归 2 项通过；包含无名称授权、身份持久化、首次/重新授权、取消/过期/冲突。
- 管理 HTTP/SQLite 回归 9 项通过；新增实际 Build 导入→二进制入库→列表邮箱投影、
  响应无 token、数据库密文不包含明文邮箱的检查。
- 前端 267 单测和 11 相关 Chromium E2E 通过，包含三尺寸、深色/辅助偏好与实际表单流程。
- 类型、相关 Clippy、权威契约同步、122-operation 四文件构建检查通过。

## 实际渠道与 Chrome 限制

先只读核对发现上一轮本地真实 Grok access token 已过期，旧存储没有身份字段。随后
对这一份本地验收账号运行既有持久 claim/CAS worker 一次：1 次 OAuth 续期、1 次
userinfo 查询成功，自动取得邮箱和用户名并写入加密 v2 载荷。账号 ID 保持不变，
revision 1→2，backoff 0，无自动重试；新 gateway 重启后管理 API 读回身份成功。
没有手工输入姓名、替换凭据、重新接受授权范围或修改生产账户。本地原加密状态已备份。
[真实渠道脱敏收据](evidence/prism-authorization-identity-20260912.json)只保留布尔、计数与 revision。

用户已告知 Chrome 没有弹窗，但连接仍返回“another extension UI is open”，重置会话
后仍复现；Chrome 原生应用连接也返回 timeoutReached。原标签页另受之前浏览器会话所有权限制，新建专用标签页仍报扩展占用。未把工具错误归因为用户未关弹窗，
也未改用其他浏览器冒充 Chrome 人工验收。真实 OAuth 续期与身份获取通过，但本轮没有完成新的 Chrome 交互闭环；
上述 Chromium E2E、真实 HTTP/SQLite 回归与真实渠道复验分别报告。

Codex 现有凭据导入/重新授权已保留 email/id_token 及刷新元数据。本轮未伪造 Claude/Kiro
尚未接通的首次 OAuth，也未为只有 API Key 或不含身份的 SSO 材料臆造人类身份。
这部分需要实际渠道授权/来源提供资料，不能以手工备注代替。

## 发布兼容性

SQL schema 仍为 24。携带身份的 Grok 加密凭据写紧凑格式 v2；无身份时仍写 v1，新版兼容
读取 v1。旧部署程序不能直接读取新写入的 v2，因此未来发布需明确格式回滚方案及备份，
不能只切回旧二进制。生产历史、凭据、域名及服务本轮均未改变。

[契约与变更说明](../change-requests/CR-PRISM-AUTHORIZED-IDENTITY-001.md)。
