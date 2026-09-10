# Prism V4 四步生产发布：限定验收通过

2026-09-10，Asia/Shanghai。用户已授权完整执行发布包、隔离演练、单渠道小测和生产升级。
本报告记录首次上线 `18f29a3be34422eb30ea7621bdf61459628b8d5f` 的验收。
后续域名入口已在c7cfd2c接通，当前运行状态见 [域名交付](prism-domain-delivery.md)。本次部署及下面列出的
新请求验收已完成；这不表示历史账务全部完成迁移，也不表示所有 Provider 均经过真实调用。

| 步骤 | 实际证据 | 结果 |
|---|---|---|
| 1 发布包 | 双架构签名构建、正式门禁通过；ARM64 独立 Cosign 身份验证、仓库 manifest/SBOM/receipt 验证、服务器 SHA256 一致 | 通过 |
| 2 隔离演练 | 生产一致副本、仅 lo 的 network namespace、降权运行；新版迁移到 22；6 个配置模型、最大授权上下文 4 个有效模型、9 个运行绑定、Prism 200；撤销 22 后旧版启动、原有表逻辑指纹一致，214 条当时已物化账本保留 | 通过 |
| 3 Grok 小测 | 复用既有 `cpar-grok-build` 客户端配置；固定 `grok-4.6`、单渠道、每请求最多 1 次 Attempt；上线前 JSON/SSE 各一次成功；响应 ID 与本机持久事件关联 | 通过 |
| 4 正式升级 | 最终停机备份、版本目录切换、运行进程二进制哈希核验；最终上线后 JSON/SSE 各一次成功；4 个成功请求各对应 1 条账本，六类 token 与 Usage 一致；管理边界、CSP、四文件、数据库检查通过 | 通过，保留以下限制 |

## 发布身份与检查范围

- [签名构建 34440246371](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34440246371)：ARM64 / x86_64 均成功。
- [正式门禁 34440249222](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/34440249222)：成功。
- ARM64 二进制 SHA256：`13a003637d39d05e527329376c78f5d89c579c8cd7a9558095bf838651abee6f`。
- Sigstore 身份绑定同仓库 `release-artifact.yml@refs/heads/codex/prism-v4-delivery`，issuer 为 GitHub Actions。
- 原 `92d1d50` 候选未上线；h2/chacha20 供应链问题修复后才使用 `18f29a3`。
  本次依赖修复的 184 项 HTTP/上游回归通过，4 项既有 ignored，cargo-audit/cargo-deny 通过。
  依据与范围见 [依赖修复报告](prism-v4-release-dependency-fix.md)。
- 本次生产浏览器核验为 Codex 内置浏览器的真实解锁页；14 栏目完整交互/尺寸/辅助偏好验收仍引用
  [V4 本地交付](prism-v4-delivery.md)及[真实本地 UAT](prism-v4-user-acceptance.md)，没有声称本轮在生产重做全部浏览器矩阵。

## 连接问题与已有 Grok 配置

此前下载出现 EOF/TLS 超时，SSH 上传出现 Broken pipe / Connection closed。
本轮复查 GitHub 直连和显式本机代理均返回 200，SSH 完成密钥认证，完整产物下载和续传上传均成功。
本机 Clash 日志明确记录：13:15 GitHub 经“Oracle 自建出口”连接 TCP 超时，13:26 出现
`timeout: no recent network activity`。因此可以确认该时段代理出口链路异常。
SSH 原始断线无法精确归因于同一跳；对应服务器 SSH 日志没有认证失败或服务重启证据。
未修改全局代理、DNS、SSH 主机校验或密钥。

既有 Pi/OMP `cpar-grok-build` 配置已有 CPAR Client Key，模型为 `grok-4.5` / `grok-4.6`。
前一轮只搜索服务器标准目录，查找范围不完整。“需要用户另给 Key”已撤销。
配置来源的 base URL 在内存中与服务器 Caddy 的 CPAR 公网 origin 比对，`/v1/models` 鉴权通过。
Key 经 SSH stdin 用于同一 CPAR，未输出、写入报告或保存为服务器新密钥文件。

## 失败、回滚与最终小测

本次不是一次无失败的发布，保留全部失败收据：

1. 首次切换失败：保护备份的 `umask 077` 使新发布目录成为 `0700`，服务用户无法执行。
   约 31.7 秒的切换任务自动恢复旧版。显式修正发布目录 `0755`，增加服务用户执行检查。
2. 随后的停机前检查误用不支持的 `--version`，被前置断言拦截，没有停机；改用实际支持的 `--help`。
3. 两次新版启动成功后的普通请求返回准入失败，第二次明确捕获 `503 CredentialUnavailable`。
   请求有 Request 事件、无上游 Attempt；两次均按约定恢复旧版。
   Explain 可选不能证明账号仍有租约容量。新增路由就绪和已验证账号的 auth/runtime/catalog/租约检查，
   并保存失败 HTTP 状态码。最终两种请求均成功。
4. 不能仅凭最终成功认定上述两次准入失败的内部根因已经完全解决；目前没有足够证据把它归因于
   并发占用、启动刷新或某一个调度条件。记录为后续需精确复现的运行问题，没有增加自动重试掩盖它。

受控请求合计 **6**：上线前成功 2、上线后失败 2、最终上线后成功 2。
4 个成功请求均为单 Attempt；两次准入失败未进入上游。保守预算收据保留失败请求的预留量，
因此其 `attempts=6` 是预算占用，不能写成已发生 6 次 Provider 调用。
没有对失败请求自动重放，没有执行其他模型/渠道自动巡检。其他客户端和既有后台 worker 的流量不计入该数字。

## 账本和历史记录的边界

最终检查时账本共有 589 行；这是检查时的总量，不是本次新增量或计费成功率。
本次 4 个成功请求各匹配一行，request/response 关联一致，六类 token 与源 Usage 逐字段一致。
input/output/reasoning/cached 已观测；cache_read/cache_creation 未观测，保留 null。
4 行费用均为 `unpriced`、金额 null；未配置价格不能解释成零费用。

另外有 **87 条升级前旧格式 Usage** 的持久 ID 仍使用 response ID，而当前读取契约使用
request 派生 ID；其中也存在同 request 多条 Usage。它们与初始备份逐字节相同，不是本次请求产生的坏数据。
当前 worker 保留这些记录并标为 `invalid_event`，未猜测选择哪条旧 Usage、改写事件或补造费用。

回滚撤销 22 号表会移除在线的失败队列，但保留已有 checkpoint。再次升级必须从失败现场副本恢复
这些待处理状态，否则会虚假显示已处理完成。本次已经在一致备份后，以原始事件哈希为条件恢复 87 条状态；
未修改事件或账本行。最终状态为 **`needs_repair` / unresolved=87**，checkpoint/source 同为 2217，
`failure_code=null`。本次新请求的物化失败数为 0。

因此：新请求的持久事件→物化→账本路径通过；87 条历史记录的兼容迁移和去重决策仍需单独处理，
现有汇总不包含它们。不能宣称“历史账本已全部补齐”或“全部消费已统计”。

## 访问与现场证据

真实管理入口：[Prism](http://127.0.0.1:18181/admin-ui/#/unlock)。本轮已建立对应 SSH tunnel，
浏览器确认显示解锁页；需要现有 Management Key 和 CSRF Token，刷新清除会话是预期行为。
隧道断开后的重连方法及回滚约束见 [生产交接](../handoffs/prism-v4-production-rollout.md)。

最终检查：18180/18181 均只绑定 loopback；匿名管理请求被拒绝；四个 SPA 文件均 200、CSP 无
unsafe-inline/unsafe-eval；SQLite quick_check/foreign_key_check 通过；Caddy 配置哈希及
Caddy/Autoreg 的进程和 active 时间均与切换前一致。未操作 Jakarta。

服务器现场：`/var/backups/cpa-rust-gateway/prism-v4-20260910-92d1d50`，权限 0700。
保留原始/最终/失败数据库、旧 binary、验签收据、隔离/回滚收据、每次切换和 canary 收据、
历史待处理恢复收据及最终 `postcheck-receipt.json`。目录中的 92d1d50 只是本次操作标识。
本机 value-free 恢复脚本位于 `output/prism-v4-rollout-20260910/`，没有把私有数据库或密钥带回本机。
