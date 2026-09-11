# CPAR 渠道管理生产发布（2026-09-11）

用户授权将当前版本部署到既有 Oracle 新加坡 CPAR。本次目标为
`https://cpar.142857142.xyz/admin-ui/`，沿用原 systemd 服务、HTTPS Origin、
loopback 监听、管理员身份及现有数据。未改动 DNS、Caddy 或 Autoreg。

已部署代码：`1cbc20dc095491ef6e47d501253e5150a9d03a2d`。
旧线上：`8a1b5377594919690c53d35a390a0c992aaf41d3`。

## 发布范围

八个主工作区；完整普通账号清单与状态操作；九类渠道导入入口；原生 Grok
账号管理，以及 Build 首次 Device OAuth 和同账号重新授权。原有设置子页面保留。
本地首次授权和重新授权已在真实 Chrome + Grok 完成；本地测试账号不会迁移到生产。

Codex / Claude / Kiro 首次 OAuth、配置热应用、批量预览提交及其余功能重构仍未全部完成。
这些限制不因本次部署而变成已完成。

## 发布检查

- 当前前端单测 264 通过、账号 E2E 7 通过、122-operation 契约/四文件双构建通过。
- 首次正式门禁发现新增三张表未加入迁移幂等测试的预期清单；补齐后定向测试通过。
  失败构建没有切换生产；以最终提交的重跑结果作为正式依据。
- 生产副本在独立网络命名空间、cpa-gateway 用户下运行，禁止外部 Provider 通信。
  schema 22→24→22、旧二进制重启和关键持久表指纹检查通过；最终签名产物复验也已通过。

## 回滚边界

切换前停服务，SQLite backup API 保存一致 control/admin-auth 快照和凭据副本，均留在
服务器私有备份目录。旧二进制和 release 目录保留。回滚先停止服务并保存完整新 schema
数据库，再逆序执行 down24 / down23，保留原有业务表与最新账本/凭据，然后恢复旧二进制。
新增加的编辑来源和原生授权审计保留于回滚前快照，不丢弃该快照；之后重升级应审查恢复。

## 实际上线结果

最终 [签名构建](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34576467734)
与 [正式交付门禁](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34576470769)
均成功。独立 Cosign 身份/issuer 校验、完整产物与 SBOM 检查通过。

ARM64 进程 SHA256：`1e625c024a0af8c764cd0a0aa63da0545a6fe3c935504f36d8ab6f6f2308c8d3`。
成功切换停服到就绪 1237 ms。schema 24；一份生效配置、5 个原生 Grok 账号、
2223 条已有事件、591 条已有账本、87 条既有未解决物化记录均保留。管理员库身份/权限
保留，没有读取或重置管理员密码。Caddy 与 Autoreg 进程/配置未变，两个监听保持 loopback。
公网四文件哈希与本次嵌入构建匹配，CSP 无 unsafe-inline/unsafe-eval，匿名管理 API
仍拒绝访问。鉴权 API 确认 9 类导入入口及 Grok Build 授权可用。

首次切换尝试在备份凭据阶段失败：systemd 停服后撤销 `/run/credentials`，脚本立即恢复
旧服务，尚未切换候选版本或迁移数据库。核对旧服务恢复后，将凭据备份移到停服前，
第二次切换成功。该失败与回滚证据保留，不将它计为成功发布。

Chrome 新开正式域名成功显示管理员登录页；没有代填或重置用户现有密码，
因此本轮 Chrome 覆盖公开登录入口，登录后功能由实际管理 API 及此前本地真实 Chrome
授权验收提供证据，并不声称重新完成所有生产交互。

服务器备份/回滚证据位于
`/var/backups/cpa-rust-gateway/prism-channels-20260911-approved`（root 私有目录）。
本机发布编排与禁止重放的 canary 预算位于 `output/prism-channels-release-20260911-approved/`。

## 公网 Grok 验收

使用既有 `cpar-grok-build` 身份，对 Grok 4.6 JSON / SSE 各请求一次，路由
max_attempts=1、无自动重试；请求携带 max_output_tokens=96，实际 Provider 返回的
token 数按原值记录（JSON output=129、SSE output=68），不把请求上限冒充实际用量。
两次响应完成并分别对应持久事件和唯一账本行；六类 token 逐字段一致，费用 confidence
均为 unpriced、金额 null。历史未解决物化数量保持 87。

[本次脱敏证据](evidence/prism-channel-production-release-20260911.json)包含实际部署、
迁移演练、门禁与 canary 收据。本次只新增正常验收调用形成的两条账本，未清理历史数据。
