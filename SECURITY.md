# Security Policy / 安全策略

## Supported versions / 支持版本

The `main` branch is the public development line.  Security fixes are handled on the latest
maintained branch; old phase branches are historical and may not receive fixes.

`main` 是公开开发主线。安全修复以当前维护分支为准；旧 Phase 分支仅作历史参考，不保证继续
获得修复。

## Reporting a vulnerability / 漏洞报告

Please do not open a public issue for credentials, OAuth material, private keys, authentication
bypass, SSRF, or production deployment details.  Use a private GitHub Security Advisory or a
maintainer-only contact channel associated with the repository.  Include:

- affected commit or version;
- a minimal reproduction without real credentials;
- impact and required privileges;
- whether a Provider request or secret disclosure is involved.

不要在公开 issue 中提交凭据、OAuth 材料、私钥、认证绕过、SSRF 或生产部署细节。请使用 GitHub
Security Advisory 或仓库维护者的私下联系渠道，并提供：

- 受影响 commit 或版本；
- 不包含真实凭据的最小复现；
- 影响范围和所需权限；
- 是否涉及 Provider 请求或 Secret 泄露。

## Credential handling / 凭据处理

Never commit API keys, access/refresh tokens, SSO cookies, passwords, private keys, production
SQLite files or client-key values.  Use ignored local files, an operating-system secret store or
deployment-managed credentials.  If a credential may have entered Git history, revoke/rotate it
first, then report the incident privately so the history can be reviewed.

绝不提交 API key、access/refresh token、SSO Cookie、密码、私钥、生产 SQLite 文件或 client-key
值。请使用 Git 忽略的本地文件、操作系统 Secret store 或部署系统的凭据注入。如果凭据可能进入
过 Git 历史，先撤销/轮换，再私下报告，以便检查历史和公开镜像。
