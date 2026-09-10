# Prism V4 新加坡生产交接

2026-09-10。后续用户明确授权四步生产发布，覆盖 `new-vps` 的 CPAR 发布、隔离验收、
最多 10 次单渠道短文本真实请求、备份和必要回滚。原 V4 本地 Goal 的“不部署”边界由这次明确授权扩展；
Caddy、DNS、防火墙、Autoreg、Jakarta 仍未列入变更范围。

## 当前状态

- 当前运行：`18f29a3be34422eb30ea7621bdf61459628b8d5f`。
- ARM64 SHA256：`13a003637d39d05e527329376c78f5d89c579c8cd7a9558095bf838651abee6f`。
- 前一版本：`4bb55b147518d32ac0ce6210ce652b4bb1668663`。
- 旧 binary SHA256：`44955dee3abde3e9a40060b19300971e64667309d78243e2143a1d7c9a3bb2af`。
- 发布包、隔离迁移/回滚、Grok JSON/SSE 和生产切换的限定验收已通过。
- 4 个成功受控请求均已物化。还有 87 条旧格式历史 Usage 待兼容迁移；worker 正确显示 needs_repair。
- 全部成功/失败次数、供应链链接、网络诊断和限制见 [发布验收报告](../reports/prism-v4-production-rollout-status.md)。

不要把本地文档 HEAD 当作已安装版本；运行身份以进程 binary SHA 和上述源码 revision 为准。
不要再要求用户提供已有的 Grok Client Key：已从既有 `cpar-grok-build` 客户端配置找到并验证。
该 Key 与服务器 Management Key、Grok Provider credential 分属不同边界，禁止互相替代。

## 管理入口

`cpa-rust-gateway.service` 仍使用现有启动参数及 credential 源，用户/组为 `cpa-gateway`，
工作目录 `/var/lib/cpa-rust-gateway`；data 18180、management 18181 均为 loopback。
完整 Prism 随 gateway binary 嵌入，不独立上传 dist。

本轮已启动本机 18181 的 SSH tunnel，并核验真实解锁页。隧道失效后，先确认本机 18181 未被其他进程占用，
再在独立终端运行：

```sh
ssh -N -T -o BatchMode=yes -o ExitOnForwardFailure=yes \
  -o ServerAliveInterval=30 -o ServerAliveCountMax=3 \
  -L 18181:127.0.0.1:18181 new-vps
```

打开 [Prism](http://127.0.0.1:18181/admin-ui/#/unlock)。保持这个 origin/端口，否则写入会被管理同源规则拒绝。
Management Key/CSRF 由操作者使用既有秘密渠道输入；禁止把值写入聊天、命令历史或浏览器持久存储。
查看结束后可在隧道终端 Ctrl-C 关闭。

## 现场及回滚

现场目录：`/var/backups/cpa-rust-gateway/prism-v4-20260910-92d1d50`，0700。
包含一致原始/最终/失败数据库、旧 binary、credential 备份、原始 argv、签名和哈希收据、
隔离/回滚/canary/切换/历史失败队列恢复/最终检查收据。92d1d50 是操作标识，不是最终安装的 revision。

版本目录 `/opt/cpa-rust-gateway/releases/<revision>`，`current` 为原子切换的符号链接。
发布目录与 binary 必须显式为 0755；创建目录的 mode 会受保护备份用的 umask 077 影响。
停机前先以实际服务用户运行 candidate `--help`；本程序不支持顶层 `--version`。

已经真实验证：旧版拒绝 schema 22。因此恢复旧版时不能只切换 binary：

1. 停止 CPAR，保留失败现场的一致数据库副本和对应收据。
2. 在单事务中撤销仅 22 号新增 failure 表/索引及 schema_migrations 登记；保留旧表中的新事件、账本和 credential 更新。
3. 切回已验证的旧 binary，启动并验证进程哈希、健康、数据库。
4. 再升级到含 22 的版本时，从失败现场恢复已知待处理队列。逐条核对源事件哈希，不回退 checkpoint，
   不以空 failure 表推断全部已完成。此次 87 条状态已恢复，原始事件和账本未改写。

若数据库完整性异常使定向撤销不成立，最终停机备份是恢复点；需要明确披露恢复点之后的写入差异。
本次未演练通用损坏数据库恢复，不应宣称已覆盖所有故障模式。

## 下次发布的验收门槛

- 使用完整签名产物；本地独立验签、服务器哈希一致后才运行。不要运行部分上传目录。
- 隔离副本使用 network namespace 仅 lo、无默认路由及服务用户权限；避免副本后台刷新与生产 OAuth 争用。
- 停机前核对 active 配置 ID/revision 与演练副本一致；最终恢复点在停机后重新取得。
- 健康通过之后，还要核对指定路由 Explain，以及实际账号 auth/runtime/catalog 和空闲租约；
  这些是观测，不保证随后没有并发竞争。失败记录必须保留 HTTP code 和闭集原因。
- 单渠道真实 canary 不自动重试；请求预算跨失败/回滚保留。本次预算占用 6，不能重置为零再跑。
- 公网 response ID 要对应本机持久事件；再核对新请求账本、token null/置信度、未定价状态、
  历史待处理数量、CSP/四文件、loopback 和周边服务未变更。

恢复脚本在本机 `output/prism-v4-rollout-20260910/` 和现场目录。它们绑定本次对象和收据，
不是可对未来版本无条件重跑的通用部署工具。后续操作须先检查当前状态，保留既有失败和备份。
