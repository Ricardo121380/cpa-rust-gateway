# Prism HTTPS domain access

## 2026-09-12: account identity release (latest)

Production runs `a243aab`, schema 24, at the existing CPAR HTTPS domain. Six account groups,
human identity projection, shared account layout and automatic Build identity capture are live.
Existing administrator passwords and data remain; EgoLite confirmed the public login entry.
See [production evidence and format-aware rollback](../reports/prism-identity-production-20260912.md).
Grok compact credentials may now be v2: a binary-only downgrade is unsafe. Prior entries below
describe earlier deployments.

## 2026-09-11: channel management release

Production now runs `1cbc20d` on the existing Oracle Singapore CPAR service and domain.
Schema is 24. Nine channel import entries, native Grok inventory and Build Device authorization
are deployed; administrator credentials and existing accounts/configuration/history are retained.
Signed artifacts, exact-revision formal gate, isolated schema upgrade/downgrade and public
Grok JSON/SSE-to-ledger canaries passed. See
[release evidence and rollback](../reports/prism-channel-production-release-20260911.md).
The V6/schema-22 statements below are historical. Other providers' first OAuth and runtime
hot apply remain incomplete; this release does not claim the entire functional plan complete.

## 当前界面与登录方式（2026-09-11）

当前运行 V6 `8a1b537`，沿用 `b30d191` 引入的管理员账号密码登录和同一 HTTPS 域名。
OpenDesign Pi／K3 max 布局、生产资源名称、签名验收及回滚点见 [V6 交付报告](../reports/prism-v6-refinement.md)。
本轮保留历史请求、账本、当前配置与原管理员认证库；此前 11 个测试草稿清理结果见 [V5 报告](../reports/prism-v5-refinement.md)。
登录页不再要求手工填写 Management Key/CSRF。默认账号 `admin`，随机初始密码已通过
私有文件交付；首次登录强制改密。见 [登录交接](prism-admin-password-login.md) 与
[实际交付报告](../reports/prism-admin-login-delivery.md)。下文 Key/CSRF 页面说明是此前域名
切换时的记录；CLI 凭据兼容、精确 HTTPS Origin 和 loopback listener 边界仍然生效。


2026-09-10. Deployed and verified for the user's cross-device manual acceptance request.
This extends the previous deployment scope only for CPAR's existing domain and management origin.
The prior loopback-only default remains available when the option is omitted.

Original domain cutover runtime: `c7cfd2c0d8771187775d2cd846cb49a8cf536046`; its predecessor was `18f29a3`.
Current runtime is recorded in the update above.
Signed artifact, public HTTPS write/readback and browser evidence: [delivery report](../reports/prism-domain-delivery.md).
The current entry is the existing approved `cpar` domain, with `/` redirecting to `/admin-ui/`.

## Service and Caddy

Run the service with its existing arguments plus:

```sh
--management-origin https://cpar.example.com
```

Use one exact canonical HTTPS origin with no trailing slash, path, query or fragment.
The management listener still binds `127.0.0.1:18181`; Caddy remains the TLS entry point.
The declared origin replaces the loopback browser origin. Origin-free authenticated CLI requests
continue to work; browser writes from the old SSH-forwarded HTTP origin are rejected.

Apply only the approved hostname's routing from
[the Caddy example](../../deploy/caddy/prism-domain.Caddyfile), preserving existing site directives
and the existing data proxy configuration. Forward Origin unchanged. Do not embed keys or CSRF
values into Caddy. Routing uses mutually exclusive `handle` blocks as described by
[Caddy's official documentation](https://caddyserver.com/docs/caddyfile/directives/handle).

The base systemd template is unchanged. For an existing installation, an operator-owned drop-in
may replace ExecStart with its existing argv plus the explicit origin; keep all current proxy
arguments and the systemd `%d` credential directory. Back up and validate before restarting.

## Acceptance

The expected user entry is `https://<approved-cpar-host>/admin-ui/`; exact `/` redirects there.
Enter the existing Management Key and CSRF Token. Client Keys and Provider credentials cannot
unlock Prism. Secrets remain only in page memory and refresh locks the page.

Verify TLS/UI/four assets, authenticated read and a dedicated unpublished draft write/readback, wrong-origin and
missing-CSRF denial, anonymous denial, the existing public `/healthz` and authenticated `/v1/models`,
and unchanged other Caddy sites/Autoreg. These checks send no Provider inference request.
Actual access on the user's other physical devices remains their manual acceptance step.

## Recovery boundary

Restore this operation's previous binary, service drop-in state and only its Caddy site change.
Both old and new artifacts have schema 22, so do not drop that migration or its failure queue.
The 87 historical legacy Usage records and the previously observed admission 503s remain separate
project follow-ups; this domain-access change does not claim to resolve them.
