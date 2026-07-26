# P12 生产配置图录入与客户端 Key 迁移 Runbook

| 项 | 值 |
|---|---|
| 状态 | v1.0 — 按 `CR-P12-ROLLOUT-001`（范围/分流层）与 `CR-P12-06-001`（组合放宽与可观测性）编写 |
| 适用 | P12-06 前置：生产配置图定义与录入、客户端 Key 迁移、Caddy 分流与回滚 |
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

## 4. 客户端 Key 迁移（含双接受窗口）

**事实**：新网关不接受外部提供的 key 值（`ClientKeyInput` 无 secret 字段，key 一律服务端随机生成）；CPA 的 key 无法导入。迁移方向只能是**全体客户端换发 `rgw_` 新 key**。

**双接受窗口**（使即时回滚成立的关键）：

1. 服务器上冻结并清点 CPA 现行 api-keys 清单（在 `/opt/example-legacy-gateway/cpa` 配置内读取；仓库无值）
2. 为每个客户端经管理 API 签发 `rgw_` key（§3 步骤 12）
3. **把同一批 `rgw_` key 值加入 CPA 配置的 api-keys 列表**——CPA 接受任意字符串 key，这一步让两侧同时接受新 key
4. 按 Canary 阶段计划把新 key 逐批交付客户端替换旧 key
5. Caddy 按字面前缀分流（见 §5）：带 `rgw_` key 的请求 → 新网关，其余 → CPA
6. **阶段百分比 = 已换发客户端的流量占比**（确定性、保缓存亲和、按 key 可归因）
7. **即时回滚** = Caddy reload 把 `rgw_` 流量也指回 CPA——因第 3 步，CPA 照常接受这些 key，客户端无感知
8. 100% 切换稳定 72h（G12）后：从 CPA 配置移除 rgw_ 值、废弃 CPA 旧 key，关闭双接受窗口

## 5. Caddy 分流与备份

### Caddy 要点

- 公网 TLS 终止后反代到 `127.0.0.1:18180`（数据面）；**不得存在任何到 18181 的公网路由**
- 分流规则：`Authorization: Bearer rgw_…` **或** `x-api-key: rgw_…`（数据面二选一互斥，两个头都得匹配前缀）→ 新网关；否则 → CPA。匹配的是**非机密**的固定字面 `rgw_`，配置中不出现任何 key 值
- 读/空闲超时必须 **> 15 秒**（数据面 SSE keepalive 间隔；`CR-P12-ROLLOUT-001` 硬性核对项）
- 回滚配置（全量指回 CPA）预先放好；P12-09 演练实测 `caddy reload` 生效时延并记为 RTO

### 备份

- **无 HTTP 备份创建/下载端点**（`create_operator_artifact` 刻意未挂载；HTTP 面只有 preflight 与 restore）。发布前备份为服务器文件系统级：静止拷贝或 SQLite 在线备份 `/var/lib/cpa-rust-gateway/control.sqlite3`，沿用 P12-03 的 tar+哈希收据格式
- `POST /admin/restores` 只恢复到配置的 `restore-target.sqlite3`（不覆盖运行库），恢复后人工切换并重启
- API 层回滚仅单步（只保留 active 的直接前驱）；更深回退走文件系统备份

## 6. 已知缺口（操作时须心中有数）

| # | 缺口 | 操作对策 |
|---|---|---|
| 1 | 无 key 导入路径 | 全员换发 `rgw_` key + §4 双接受窗口 |
| 2 | 发布后需重启（无热加载） | 发布排低谷；Canary 窗口内冻结配置变更 |
| 3 | endpoints/credentials/aliases/routes/candidates 无 list | §2 台账强制 |
| 4 | `/admin/endpoints/{id}/test` 在生产组合恒 `rejected` | 真实数据面请求 + explain + availability 验证 |
| 5 | 无 HTTP 备份创建 | 服务器文件系统级备份（P12-03 流程） |
| 6 | API 回滚仅单步 | 深回退走文件系统备份 |
| 7 | 管理面按对端地址放行 | Caddy 审查断言无 18181 公网路由 |
