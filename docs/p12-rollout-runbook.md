# P12 生产配置图录入与客户端 Key 迁移 Runbook

| 项 | 值 |
|---|---|
| 状态 | v1.1 — `CR-P12-ROLLOUT-002` 覆盖百分比分流，采用 CPAR 全量替代 CPA |
| 适用 | 生产配置图定义与录入、客户端 Key 迁移、Caddy 全量切换与回滚、旧 CPA 下线 |
| 事实基线 | 全部操作步骤与约束均核对自当前代码与 `docs/openapi/management-v1.json`；服务器侧数值（CPA key 清单等）仓库刻意不存，须在服务器上读取 |

## 1. 前提与不变量

- 数据面与管理面都只绑回环：`gateway serve --data-listen 127.0.0.1:18180 --management-listen 127.0.0.1:18181 --state-dir /var/lib/cpa-rust-gateway --credential-dir %d`。两个监听器必须不同、非零端口、回环地址，否则拒绝启动（`deployment.rs` parse）。
- 凭据目录须含五个文件（直属常规文件、非符号链接、`O_NOFOLLOW` 打开）：`management-key`（`mgmt_` 前缀，32–512 字节 ASCII）、`management-csrf`（`csrf_` 前缀）、`master-key`（恰 32 原始字节）、`backup-key`（恰 32 原始字节）、`client-key-pepper`（恰 32 原始字节）。服务器上 root:root 0600 存于 `/etc/cpa-rust-gateway/credentials/`，经 systemd `LoadCredential=` 交付。
- **发布后无热加载**：数据面在进程启动时从 active 配置版本一次性组合；任何 publish/rollback 之后运行中的进程 fail closed，必须 `systemctl restart cpa-rust-gateway`。每次发布都伴随一个短暂数据面窗口，须排在流量低谷。
- **管理面绝不公网暴露**：网络策略按对端地址判定（LoopbackOnly，忽略转发头）。同机 Caddy 若误配一条到 18181 的公网路由，对端会呈现为回环并被放行（仍需管理 Key）——**Caddy 配置审查必须显式断言不存在任何代理到 18181 的公网 vhost/路径**。
- 管理请求头协议：所有请求带 `X-Management-Key`；图资源读写另带 `X-Config-Version: <版本id>`；所有变更带 `If-Match: rev-N`（成功响应的 `ETag` 给出下一个 rev）；无 `Origin` 头的 curl 不需要 CSRF token。

## 2. 操作台账（强制）

以下资源**没有 list 端点**：endpoints、credentials、aliases（完全无读端点）、routes（无集合列表）、candidates（完全无读端点）。ID 一旦丢失，API 层无找回路径。**从第一条调用起维护台账**，记录：`config_version_id`、`egress_policy_id`、`upstream_id`、每个 `endpoint_id`、`credential_id`、`public_model_id`、`alias`、`route_id`、`candidate_id`、`access_group_id`、`client_key_id` 与其**公开前缀** `rgw_<16hex>`，以及每次变更后的当前 `rev-N`。

## 3. 生产配置图录入序列（空库 → 发布）

全部在管理监听器 `127.0.0.1:18181` 上执行；步骤 2–12 携带 `X-Config-Version` + 当前 `If-Match`。

1. `POST /admin/config-versions` `{id, description}` → 201，得初始 revision
2. `POST /admin/egress-policies`——schemes 仅 `["https"]`、逐一列出上游主机与端口、`redirect_mode:"deny"`、`max_redirects:0`
3. `POST /admin/upstreams`（每个中转站一条，引用出口策略）
4. `POST /admin/upstreams/{id}/endpoints`——`adapter_id` 必须与 `api_format` 配对（`openai-compatible.responses`↔`openai/responses`，`anthropic-compatible.messages`↔`anthropic/messages`）；词表之外的 `api_format` 在 validate/publish 阶段即以 `unsupported_endpoint_api_format` 整版拒绝，本 build 未绑定适配器的格式在装配阶段 fail closed、`base_url`、`inference_path`
5. `POST /admin/upstreams/{id}/credentials`——`kind:"bearer"`、`secret` 为一次性写入明文（服务端 XChaCha20-Poly1305 加密后落库；GET 只回元数据）
6. `POST /admin/endpoints/{id}/credential-bindings`——priority/weight/concurrency 按容量规划
7. `POST /admin/public-models`（每个公开模型一条）
8. `POST /admin/public-models/{id}/routes`——`policy:"smooth_weighted_round_robin"`、`max_attempts` 在放宽后上限内（重试均发生在首字节前）、`bootstrap_timeout_ms ≤ 15000`
9. `POST /admin/routes/{id}/candidates`（每条路由的每个上游候选一条；`transform_mode:"canonical"`）
10. `POST /admin/public-models/{id}/aliases`（如需别名；录入后无读端点，全靠台账）
11. `POST /admin/access-groups` + `POST /admin/access-groups/{id}/routes`
12. `POST /admin/client-keys` `{id, access_group_id, status:"active"}` → **201 响应含一次性完整 key（`rgw_<16hex>_<64hex>`，85 字符）与非机密前缀**；key 只显示这一次，当场交付客户端并记录前缀
13. 每条路由 `POST /admin/routes/{id}/validate` + `GET /admin/routes/{id}/explain?requested_model=…&protocol=openai_responses`
14. `POST /admin/config-versions/{id}/validate` → `{valid:true}`
15. **发布前备份**（见 §5）
16. `POST /admin/config-versions/{id}/publish`（带 `If-Match`）→ `systemctl restart cpa-rust-gateway`

### 发布后验证

`POST /admin/endpoints/{id}/test` 在生产组合中固定返回 `rejected`（RejectingManagementEndpointWorkflow），**不能**用于连通性验证。改用：

- 持签发 key 请求 `GET /v1/models`——应恰好返回该 key 访问组可见的模型集（快照过滤）
- 一次真实 `POST /v1/responses`（非流式，小请求）
- `GET /admin/routes/{id}/explain` 与 `GET /admin/runtime/availability` 对照选路
- `GET /admin/requests/{request_id}/attempts` 确认事件管道产出真实 attempt 记录

## 4. 客户端 Key 迁移（直接替代）

**事实**：新网关不接受外部提供的 key 值（`ClientKeyInput` 无 secret 字段，key 一律服务端随机生成）；CPA 的 key 无法导入。迁移方向只能是**全体客户端换发 `rgw_` 新 key**。

1. 服务器上冻结并清点 CPA 现行 api-keys 清单（在 `/opt/example-legacy-gateway/cpa` 配置内读取；仓库无值）。
2. 为每个实际客户端经管理 API 签发独立 `rgw_` key（§3 步骤 12），建立操作者持有的交付台账。
3. 每个客户端在切换窗口内一次性把 endpoint 与 key 从 CPA 改为生产主机名上的 CPAR 组合；Caddy
   同时把该生产主机名全量改到 CPAR。不得让 Caddy 按 key 或百分比把生产请求拆到两侧。
4. 回滚时先把 Caddy 全量恢复到 CPA，再按台账恢复客户端旧 key。因为 CPAR 当前不能导入 CPA key，
   也不再要求 CPA 接受 `rgw_`，所以回滚不是无感 key 回滚；客户端恢复耗时必须计入 RTO。
5. CPAR 全量稳定 72h 且 G12 通过后，停止并禁用旧 CPA service/container；保留加密备份和回滚包，
   但服务器生产入口只保留 CPAR。

## 5. Caddy 全量切换与备份

### Caddy 要点

配置模板在仓库内：全量切换用 [`deploy/caddy/canary.Caddyfile`](../deploy/caddy/canary.Caddyfile)，
回滚 preimage 用 [`deploy/caddy/rollback.Caddyfile`](../deploy/caddy/rollback.Caddyfile)。
两者都是**片段**，只替换生产主机名那一个站点块，其余站点（cpam/grok/kiro/sub）不动。

- 公网 TLS 终止后反代到 `127.0.0.1:18180`（数据面）；**不得存在任何到 18181 的公网路由**
- 全量切换规则：生产主机名唯一 `reverse_proxy 127.0.0.1:18180`，不得出现按 key、header、权重或
  百分比的生产分支，也不得保留到 CPA 的 fallback。
- **不要加全局 `servers` 超时块**。Caddy 的 `servers` 是按**监听地址**生效的，不是按站点；服务器上
  五个站点共用同一个 `:443` 监听器，加全局块会把超时施加到 cpam/grok/kiro/sub 上（已用
  `caddy adapt` 实测确认）。要按站点隔离只能换端口，那会改变公开面
- 现行配置编译出来是 `timeouts: NONE`，即 Go 的"无超时"默认值——这对长连接 SSE 恰好是正确的，
  也天然满足 `CR-P12-ROLLOUT-001` 的"读/空闲超时 > 15 秒 keepalive 间隔"。**风险方向与直觉相反**：
  这里的危险不是缺超时，而是有人按常规运维直觉**加**一个 read/idle 超时，那会在两次 keepalive
  之间切断健康的空闲流。`scripts/check-p12-caddy-split.rb` 因此既拒绝全局 `servers` 块，也在
  真有超时被引入时断言它仍高于网关自身上限（从 Rust 源码读取：15s keepalive、30s 入站正文、
  15min 进度截止、1h 流式总上限）
- **不要给数据面站点加 `encode`**：压缩会在事件流前重新引入缓冲。Caddy 对
  `Content-Type: text/event-stream` 本身就立即 flush，无需额外配置
- 回滚配置（全量指回 CPA）预先放好；P12-09 用
  [`scripts/p12-09-measure-caddy-rto.sh`](../scripts/p12-09-measure-caddy-rto.sh) 实测生效时延并记为 RTO。
  注意 `caddy reload` 返回零只表示配置被接受，**不**表示下一个请求已走新路由，所以脚本轮询探针
  直到实际观测到后端改变，分别记录 `reload_returned_ms` 与 `effective_ms`

### P12-07 暴露前验证域名

[`deploy/caddy/staging-domain.Caddyfile`](../deploy/caddy/staging-domain.Caddyfile) 是新网关的
**第一条公网路由**，只用于全量切换前验证 DNS/TLS/认证与管理面边界，P12-09 切换后即应移除。
它只反代数据面 18180，
同样不含全局 `servers` 块、不含 `encode`、不含到 18181 的任何路由。

验证用 [`scripts/p12-07-verify-exposure.sh`](../scripts/p12-07-verify-exposure.sh)，八项断言全部
fail-closed：权威 NS 解析、非代理（灰云）、证书主机名匹配、数据面可达、**未认证请求被拒**、
**错误 key 被拒**、管理面四条路径均非 200、以及（给了 key 文件时）**合法 key 被接受**。
最后一项不可省：否则前两项负向检查在一个"拒绝一切"的坏路由上同样会通过。

**已知缺口——无限流**：`CR-P12-ROLLOUT-001` 为该域名列了限流，但服务器上的 Caddy 是标准版，
`caddy list-modules` 实测**没有 rate_limit 模块**。补偿控制按实际约束力排序：数据面每条路由
都要求 client key（未认证在触达上游前即被拒）、入站正文上限 4 MiB 且读取 30s 有界、
绑定总并发上限 16、该主机名不对任何客户端公布且 P12-07 完成后移除。加限流需要自定义 Caddy
构建，会改动 incumbent 的 TLS 终止器，不在暴露前检查的范围内。

### 备份

- **无 HTTP 备份创建/下载端点**（`create_operator_artifact` 刻意未挂载；HTTP 面只有 preflight 与 restore）。发布前备份为服务器文件系统级：静止拷贝或 SQLite 在线备份 `/var/lib/cpa-rust-gateway/control.sqlite3`，沿用 P12-03 的 tar+哈希收据格式
- `POST /admin/restores` 只恢复到配置的 `restore-target.sqlite3`（不覆盖运行库），恢复后人工切换并重启
- API 层回滚仅单步（只保留 active 的直接前驱）；更深回退走文件系统备份

## 6. 已知缺口（操作时须心中有数）

| # | 缺口 | 操作对策 |
|---|---|---|
| 1 | 无 key 导入路径 | 全员换发 `rgw_` key + §4 客户端交付/回退台账 |
| 2 | 发布后需重启（无热加载） | 发布排低谷；全量观察窗口内冻结配置变更 |
| 3 | endpoints/credentials/aliases/routes/candidates 无 list | §2 台账强制 |
| 4 | `/admin/endpoints/{id}/test` 在生产组合恒 `rejected` | 真实数据面请求 + explain + availability 验证 |
| 5 | 无 HTTP 备份创建 | 服务器文件系统级备份（P12-03 流程） |
| 6 | API 回滚仅单步 | 深回退走文件系统备份 |
| 7 | 管理面按对端地址放行 | Caddy 审查断言无 18181 公网路由 |
| 8 | **服务端无延迟分位数**：Prometheus 暴露面 7 个指标全是计数器，零 histogram；`ManagementRequestAttempt` 按设计不含 timing | TTFT 与 P95/P99 由客户端侧探针采集；Attempt 级时长由事件日志 `started_at_ms`/`ended_at_ms` 离线导出统计。**不得**声称服务端提供实时分位数 |
| 9 | `attempts_total` 只有 `succeeded`/`failed` 两个标签值，无 HTTP 状态码或错误分类维度 | 错误率分子按客户端观测状态码 + Attempt 载荷的 `GatewayError` 分类共同判定 |
| 10 | Tool 与 Route 分布无指标 | 按 §2 台账逐样本核对，不按指标聚合 |
| 11 | **入口无限流**：服务器 Caddy 标准版无 `rate_limit` 模块 | 依赖 client key 强制、4 MiB 正文上限、30s 正文读取上限、总并发 16、测试域名不公布且用后移除；加限流需自定义 Caddy 构建，另行 CR |

## 7. 全量替代判据（操作口径）

完整定义见计划 §18；此处只给操作时要盯的三件事。

- **样本量**：CPAR 全量窗口至少 **1250** 个成功请求；切换前冻结 CPA 基线，不要求两者并行承载
  生产流量。合成补足请求单独计数，其失败同样计入分子。
- **时长**：CPAR 全量生产观察至少 **72h**；P12-09 先完成一次全量回滚/恢复演练。
- **回滚判定**：按计划 §18 的四级严重度表。P0（数据/隔离破坏）与 P1（全量降级、语义回归、
  必需事件被隔离或必需队列满）立即回滚且 G12 不通过；P2/P3 记录但不阻塞。每阶段结束前
  至少核对一次 `durable_events_total{outcome=required_quarantined}`、`{outcome=write_failed}` 与
  `queue_admission_total{outcome=required_queue_full}`、`{outcome=sink_closed}` 是否增长——这四个
  是 P1 的机械信号（标签拼写与 `crates/gateway-observability/src/telemetry.rs` 一致）。
