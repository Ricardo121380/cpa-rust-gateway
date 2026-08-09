# P12 Codex production deployment and acceptance review

日期：2026-08-09

## Scope

本次操作获得 operator 的明确授权，将已在 Oracle Singapore 隔离 staging 通过完整
12-tuple 的 ARM64 本机构建发布到公网 CPAR 生产数据面，并使用现有生产 client key
完成真实 Base URL 验收。没有修改 Caddyfile、DNS、管理监听、正式凭证内容或旧 CPA。

## Deployment evidence

- 目标 release：`a473a385461bb606246cafafe892f9d0a8f4c6dd721fe7dba8322889be460244`
- 架构：Linux ARM64 ELF
- 来源：已在隔离 staging 运行的同一构建；staging receipt 为
  `e2e-codex-bridge-v2-fresh-20260809.json`
- 当前生产 release 在切换前已独立保存；回滚包：
  `/var/backups/cpa-rust-gateway/p12-codex-deploy-20260809T120526Z`
- SQLite 在线备份 `PRAGMA quick_check=ok`
- 目标二进制复制后 SHA-256 与 staging 完全一致
- 新进程重启后 `active`，本地 `/healthz` 为 HTTP 200
- 生产 client-key 的本地 `/v1/models` 为 HTTP 200
- 公网 health 为 HTTP 200，公网管理路径未暴露
- active Config Version 仍为既有生产版本；Caddyfile 字节哈希在切换前后相同
- preflight 使用生产数据库快照在独立 loopback 端口加载新二进制，`/v1/models` 和一条
  Chat JSON 均为 HTTP 200；preflight 进程和临时凭证目录已清理

## Runtime acceptance

### Isolated staging

当前 OAuth 修复构建在隔离 staging 通过真实 CPAR data listener + staging client key：

```text
fixed=12 attempted=12 successful=12 network_send=12
Chat/Responses/Messages × JSON/SSE × text/tool
```

### Public Base URL

公网验收使用真实公网 Base URL + 生产 client key，采用同一 value-free harness，单轮首败停止：

- 入口预检：`/v1/models` HTTP 200
- 最后一轮：Chat JSON text、Chat SSE text、Chat JSON tool 通过；Chat SSE tool
  在客户端收到 `chat_stream_error_frame` 后停止，计数为 `3/4`
- 前一轮的首败位置为 Chat SSE text；在本机 18180 直接请求同形 text/tool SSE 均通过
- 公网单次诊断曾完整收到终帧、usage 和 `DONE`；诊断不计入验收 tuple
- 生产日志对相关 upstream attempt 记录为 `succeeded`，没有 `CredentialUnauthorized`
  或入口认证失败

公网 receipt（root-only）保留在部署回滚目录：

```text
/var/backups/cpa-rust-gateway/p12-codex-deploy-20260809T120526Z/public-codex-12tuple-receipt-final.json
```

## Verdict

`DEPLOYED_WITH_BOUNDARY / BLOCKED_PUBLIC_SSE_INTERMITTENT`

新构建已部署且本机/隔离 staging 的 OAuth runtime 证据通过；公网完整 12-tuple 尚未
达到 PASS。失败位置在不同公网轮次漂移，而本机同形请求通过，且部署前旧生产二进制
也出现过相同的公网 SSE 首败，因此当前证据更支持公网链路/上游响应的间歇性问题，
不能归因于本次 OAuth 代码变更。

本次没有自动回滚：回滚会恢复已知存在同类公网失败的旧二进制，不能消除已观测问题；
但生产状态不得标记为“公网验收通过”。若要关闭该边界，下一步应从外部客户端或
独立出口复核 SSE framing/Caddy/网络路径，再登记一次单独的受控公网验收；不应先修改
OAuth 请求轮廓或替换凭证。

## Provenance boundary

该 release 是 Oracle ARM64 本机构建，未运行 GitHub CI、未生成新的 GitHub OIDC/Sigstore
release receipt。它满足本次 operator 明确要求的部署与 staging 验收，但不应被记录为
正式 signed release-artifact closeout。
