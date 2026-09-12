# 账号身份与分组生产发布（2026-09-12）

用户在 EgoLite 本地验收后明确授权上线。已将
`a243aabe66ea456d158afcc3fa62e95f49d3ee82` 部署到既有 Oracle 新加坡 CPAR，替换
`1cbc20dc095491ef6e47d501253e5150a9d03a2d`。正式入口为
[Prism 管理端](https://cpar.142857142.xyz/admin-ui/)。管理员仍为既有账号及密码，
未读取或重置密码，未将本地合成或真实验收账号导入生产。

## 本次上线内容

全部账号按 API、Codex / ChatGPT、Claude、Kimi、Kiro、Grok 分组，Grok 内含
Web / Console / Build；共用列表与手机布局。账号身份使用服务端观测到的邮箱、电话
或用户名，Autoreg 作为来源；接口连接显示协议，并在详情说明主机和配置状态。
详情及重新授权弹窗沿用身份，内部 ID 收在技术详情。

Grok Build 在授权、支持的导入与续期中保留身份；缺失时使用现有授权向固定 issuer
读取 userinfo，并核对 subject。紧凑凭据 v2 将身份保存在加密载荷中，兼容读取 v1。
SQL schema 仍为 24，现有认证、配置与历史数据保留。

首次生产 API 读回：3 个普通账号中 2 个有身份资料，5 个原生 Grok 账号中 1 个有身份。
这是当次读取结果，不是可调用或在线账号数。旧凭据没有身份时仍显示未提供身份，
不会由历史 ID 或导入标记推断邮箱；支持的 Build 后续授权/续期会自动取得可用身份。
裸 Web / Console SSO 不保证提供身份资料。

## 发布验证

- [签名构建](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34679527020)
  两个 Linux 架构通过；ARM64 二进制、SBOM、manifest、receipt 及 Cosign 签名已独立验证。
- [正式交付门禁](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34679537441)
  在同一提交通过，包括 Fast 和供应链检查。
- 新 ARM64 二进制以合成状态通过真实管理员初始化、首次改密、Origin/CSRF、会话注销和
  重启检查；所有四个嵌入文件与本地已验收构建哈希一致。
- 生产状态副本在独立网络命名空间、服务用户下启动新版；3 个普通账号与 5 个 Grok
  账号可读，新的身份/分类/连接字段存在。副本没有外部网络，不发真实 Provider 请求。
- 隔离副本中两条 v1 凭据加入合成身份并加密为 v2，新版管理 API 实际读到该身份；
  再降级为 v1、重启旧二进制通过。令牌字节及其他数据表保持一致，重复降级无改动。
- 成功切换停服到就绪 **1245 ms**。实际进程 SHA256 为
  `037f46b77131bc8241cbb1c7dfb14455f388ea0b9a0ce7688f6247a51e9f7d5b`。
  切换前已有 2229 条事件、593 条账本、87 条未解决物化记录均保留；5 个原生账号、
  当前生效配置、管理员库与先前 11 个测试草稿的清理审计保留。
- 服务器及操作者网络分别核对公网四文件哈希；健康检查通过，CSP 不含
  unsafe-inline/unsafe-eval，匿名管理请求仍拒绝。Caddy、DNS、Autoreg 与 loopback 监听未变。
- EgoLite（Chromium 152）实际打开正式 HTTPS 域名，管理员登录页显示正常，并留给用户
  人工验收。没有代填密码，因此本次生产浏览器验证覆盖登录入口；登录后的能力由
  [本地 EgoLite 验收](prism-egolite-20260912.md)和生产鉴权 API 读回分别佐证。

本次没有额外发起真实授权、推理 canary 或强制刷新；正常服务续期仍按既有调度执行。
前一轮本地真实 Grok 身份获取证据见 [授权身份报告](prism-authorization-identity-20260912.md)。

## 备份与回滚

服务器私有目录：`/var/backups/cpa-rust-gateway/prism-identity-20260912`。
切换前先复制 systemd 凭据，再停服务，用 SQLite backup 保存一致控制库及管理员库。
旧 release 目录保留。回滚不能只换旧二进制，也不能用旧数据库覆盖新账本或轮换后的令牌。

本轮的 `compact_rollback.py` 在停服并备份当前库之后，通过现有 AEAD 格式验证及解密，
只转换 `grok_accounts` 的 Build 凭据和 `grok_build_credential_runtime` 中的紧凑 v2：
保留当前 access/refresh token、期限、来源、client、scope，以及逻辑 revision，移除旧版
不认识的尾部身份显示段后使用新 nonce 重加密。两个表在一个排他事务内完成；未知版本、
认证失败或格式异常会拒绝提交。身份仍保留在回滚前的加密快照中。

该工具及部署脚本保存在上述私有目录；精确脚本哈希随证据记录。工具使用固定版本
PyNaCl 1.6.0、cffi 1.17.1、pycparser 2.22 的官方 PyPI wheel，核对 SHA256 后离线安装
到该目录的 `python-deps`，没有修改系统 Python 或应用依赖。降级演练只发生在隔离副本，
生产没有执行降级。没有数据库 schema 升降级或历史删除。

本机编排与完整临时日志在 `output/prism-identity-release-20260912/`；
[提交的脱敏证据](evidence/prism-identity-production-20260912.json)包含构建、隔离、回滚、
线上读回和 EgoLite 结果。旧渠道首次 OAuth 等功能计划的未完成项不因本次发布变成完成。
