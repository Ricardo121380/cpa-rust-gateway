# 项目审查：后端与 Prism 前端

日期：2026-09-09。审查基线：`codex/p13-account-tier`，`1ba4340b56e9f4214c98912f78dc477fdc817ea2`。

建议保留现有 Rust 分层与 Prism 视觉基础，优先补齐运行装配、数据增长边界和前后端契约接线。当前最重要的问题不是缺少页面，而是库中的能力尚未全部进入 `serve`，以及新管理字段没有进入前端。

本报告是审查结果，不是实施完成记录。新开发方案见 [Prism 下一阶段开发方案](../handoffs/prism-development-plan-2026-09-09.md)。本次未修改应用代码、契约或原有计划状态，未访问生产服务器或发送 Provider 请求。

## 1. 范围与证据

- 正式项目是 `cpa-rust-gateway/`；正式前端是 `web/prism/`，由 Rust 构建脚本编译后嵌入。外层 `../prism/` 是较早的独立 Git 仓库，不能作为当前开发基线。
- 覆盖 workspace 结构、部署装配、HTTP 管理与数据边界、路由/目录/凭据调度、协议转换、流式背压、鉴权、持久化/计费、Prism 的页面和 API 接线、构建检查以及交接记录。采用结构扫描、关键调用链追踪、定向测试和本地页面查看；不是对约 15.9 万行 Rust 源文件逐行穷举，也不是生产容量或完整安全审计。该行数包含源文件内的测试。
- 已读取项目 `AGENTS.md`、`CLAUDE.md`、Oracle Singapore handoff、旧前端设计/计划、Prism `DESIGN.md` 相关章节，以及跨边界日志中账号权益、目录、自动续期和近期 Responses 修复的交接。
- 当前前端有 12 个管理页面与解锁入口。源代码最后一笔前端提交是 `d75ab21`（2026-08-24）；后端交接更新到了 2026-09-08。
- 用户已有的 `.codegraph/` 与四个未跟踪辅助脚本保持原状；未读取凭据目录、生产数据库或 SSH 配置。

## 2. 后端：需要优先处理的发现

以下优先级是本次审查的排期建议：P1 应在相关能力继续作为可用产品交付前解决；P2 是明确的正确性或持续运行问题；P3 是后续优化。源码推导与实际测试分别说明。

### B1 · P1 · 计费物化没有进入 `serve` 的运行链路

**位置**：`apps/gateway/src/deployment.rs:234`、`crates/gateway-control/src/billing_materializer.rs:94`。

`materialize_billing_events` 已实现从持久事件、价格目录和请求归属信息生成账本，也有 checkpoint/幂等测试。但当前源码中的调用都在其单元测试内；`serve` 启动的是事件写入器、凭据刷新 worker 和模型目录 worker，没有计费物化 worker。生产装配的 `list_billing` 只读账本，不触发物化。

**触发与影响**：从空账本启动当前 `serve` 后，即使推理生成了 Request/Attempt/Usage 事件，也没有自动把这些事件写入计费账本的应用链路。价格库 CRUD 和前端账本页可以存在，但不代表真实请求会产生账本行。不能把空账本解释为零消费。本结论来自本地装配与调用图，未查询生产数据库。

**建议**：在组合根增加一个有界、可停止、使用现有 checkpoint 的账本消费者，从已持久化的事件表按 ordinal 增量读取；SQLite 工作放在 blocking 执行边界。保持请求路径不等待计价。公开安全的物化进度、滞后和失败状态，帮助 UI 区分“没有记录”和“处理未完成”。接入前核查坏记录如何避免永久卡住 checkpoint。

**验收**：受控本地请求完成后出现一条对应账本；重启及重复消费不重复收费；没有价格时仍写入 `unpriced`；处理滞后可见。现有 2 项物化单测通过，只证明库行为，未覆盖上述 `serve` 装配。

### B2 · P1 · 运营查询存在全局 100,000 条上限，筛选和分页无法绕过

**位置**：`apps/gateway/src/deployment.rs:297`、`:311`、`:328`；`crates/gateway-control/src/management_operations_service.rs:34`、`:868`、`:1353`、`:1495`；`crates/gateway-store/src/billing_ledger.rs:466`。

usage、billing、failure feedback 均先从全局存储读取 `MAX_USAGE_EVENTS + 1`，再执行时间/账号等筛选。常量是 **100,000**。聚合器或 facade 看到第 100,001 条就返回 `SourceUnavailable`，即使用户只查询最近一分钟或 `limit=1`。Usage/失败侧上限按事件行计数，不是请求数；一个请求可以产生多条事件。

账本读取还先查所有 ID，再逐 ID `load_entry`，构成 N+1 次查询。HTTP 管理 handler 同步调用这些读取与聚合；管理监听器只有一个 worker，因此大查询期间同一管理监听器的其他请求也会等待。数据监听器是独立 worker，不能据此声称所有推理请求都会被该查询直接阻塞。

**建议**：把时间、身份、snapshot 上界和 keyset cursor 下推到 SQL；账本整行批读，摘要按同一 snapshot 单独聚合；usage 使用已有持久事件建立增量投影，避免每页重建全量 Request/Attempt/Usage 关联。保留有界返回与完整性检查，不以增大常量或截掉历史来掩盖问题。耗时管理读取进入有并发上限的 blocking 执行边界。

**验收**：合成数据分别覆盖 99,999、100,000、100,001 和更大规模，窄时间窗仍可查；多页摘要不漂移；缺失事件仍显式报告；查询成本主要跟结果窗口而非数据库总量增长。当前证据是源码路径，未做生产规模压测。

### B3 · P2 · 目录的 72 小时硬过期只在 worker 发布时更新到路由

**位置**：`apps/gateway/src/runtime.rs:3412`、`:3525`、`:3595`、`:3669`；`crates/gateway-router/src/route_snapshot.rs:676`；`crates/gateway-router/src/credential_scheduler.rs:133`。

目录 worker 每轮采集一个时间值，串行检查各 target，最后用这个轮次开始时间发布 snapshot，再睡一小时。进入路由的 `SnapshotCredentialCatalog` 只有 `state`，没有过期 deadline；Candidate 是否 hard-eligible 依赖发布时的枚举状态。

**触发与影响**：最后一次发布发生在硬过期前，随后跨过 `observed + 72h`，数据面仍持有原来可用的 Stale snapshot，直到下一轮成功发布。间隔可接近一小时并叠加本轮执行时间；worker/存储持续失败时没有这个一小时上界。管理侧按当前时间读目录时可能已经显示 Expired。现有契约 `BC-CATALOG-002` 明确规定到达 72 小时即不再 hard-eligible。

**建议**：给不可变目录准入证据携带硬过期时间，在新选择/租约边界用统一时钟拒绝到期证据，不在路由热路径读 SQLite；worker 在最近 deadline 处做失效发布，发布时重新取时间。全网发现仍可保持有界周期，不需要为了失效而频繁访问 Provider。

**验收**：可控时钟跨过 72 小时后，即使 discovery 阻塞或失败，也不能新租用仅由该过期证据允许的 Credential；已有在途请求保持其既有快照语义。当前结论为跨调用链推导，未进行真实 72 小时运行实验。

### B4 · P2 · TTL 存储具备清理函数，但没有运行时清理 owner

**位置**：`crates/gateway-store/src/stored_response.rs:675`、`:923`、`:955`；`crates/gateway-store/src/billing_ledger.rs:486`；`apps/gateway/src/deployment.rs:234`。

Stored Responses 有固定 30 天 TTL 和 `purge_expired`，compaction 有 `purge_expired_compactions`，账本也有有界清理函数。当前仓库中这些清理函数没有应用运行时调用，只有测试。到期读取会隐藏记录，但这不等于密文和数据库行已经物理删除。

**影响**：持续使用存储响应/compaction 时，过期数据会继续占用数据库和备份空间。账本接入 B1 后同样需要 retention 的运行装配。

**建议**：为已定义 TTL 的表接入单一、有批量上限的维护任务，记录安全的删除数量/滞后/失败指标；避免在 HTTP 请求中做全库清理。事件日志归档及价格目录历史引用有各自约束，需要另行定义，不能一并删除。

**验收**：时间推进后只清理到期记录；有效 continuation 可继续读取；重启后维护恢复；锁竞争不会阻塞管理或推理 worker；失败不会伪报清理成功。

## 3. 后端：优化顺序与保留项

| 优先级 | 优化 | 证据与取舍 |
|---|---|---|
| 随 B2 | 消除账本 N+1、把筛选/游标下推、隔离耗时 SQLite | 已有明确调用证据。先做合成数据基准，再选择索引，不先引入新数据库或缓存服务。 |
| P3 | 让 HTTP client 的构造也进入缓存 singleflight | `gateway-upstream/src/upstream_client.rs:505` 在 `get_with` 之前就构造了 `built`；并发 cache miss 仍会重复构造 client。可用现有缓存库的 fallible 初始化机制包住构造；收益主要在冷启动/新 profile 高并发，需专项基准，不能承诺吞吐提升比例。 |
| P3 | 按职责拆分两个组合文件 | `runtime.rs` 13,374 行、`management_resources.rs` 9,809 行，均包含测试。优先在相关功能修改时拆出 catalog worker、协议 transport/decoder、management operations handlers，不以行数为由全仓重构，也不把组合根逻辑下沉到 core。 |

值得保留的设计：21 个 workspace package 的单向边界、路由不可变 snapshot、Credential 精确租约、DNS-pinned client/profile 隔离、流式背压与首次语义事件后的重试边界、HMAC Client Key 和 AEAD 存储。定向测试为这些边界提供了证据，当前没有理由更换 Rust/Actix、引入 Redis 或重写调度器。

能力完成度仍要分层：当前自动目录 worker 只组装 Build/Codex，`RuntimeCatalogProvider` 只有两个分支；其他 channel source、P13-15E 隔离与正式 Gate 仍是待办。近期 Grok Build/Responses 的成功交接不能扩展为所有渠道都可用。P13-12 权益、P13-16 自动续期的阶段状态，以当前后端计划及各自验收边界为准。

## 4. 前端：已完成部分和实际缺口

### 已有能力应继续使用

| 类别 | 已有实现 | 下一步重点 |
|---|---|---|
| 配置生命周期 | 草稿、校验、发布、回滚、ETag | 修复响应与版本归属；改善导航上下文。 |
| 上游与访问 | 上游/Endpoint/Credential/绑定、访问组、Client Key | 把配置记录与实时账号状态连起来；保留写入限制。 |
| 模型路由 | Public Model CRUD、Route 工作台、候选创建、Explain | 缺少完整枚举和候选修改/删除；有效模型目录需要新只读投影。 |
| 运营 | Usage 聚合、账本、失败归因、请求 attempts、价格目录 | 保留数据置信度；先解决后端 B1/B2，再做进一步分析。 |
| 运行诊断 | 可用性矩阵、账号池、三域出口状态、Channel Pin | 页面分区，接入权益和目录新字段。 |
| 交付与视觉 | Liquid Glass、暗色、降级、窄屏、无浏览器存储、四文件嵌入 | 以可读性、任务路径与真实契约验收为主。 |

现有 99 个 operation 中，生产源文件出现 87 个调用字面量；另外 12 个仍与旧计划的明确不做项一致。这是接口接线统计，不是字段、状态或端到端完成率。

### F1 · P1 · 契约不同步，新交接字段未被消费

**位置**：`web/prism/contracts/management-v1.json`；`web/prism/src/features/runtime/model.ts:274`；`web/prism/src/features/runtime/RuntimePage.tsx:491`、`:910`。

权威 OpenAPI 与 vendored 副本存在三个 schema 差异：`CatalogStatus`、`ProviderAccountPoolItem`、新增的 `ProviderAccountEntitlement`。账号池未展示 entitlement；目录仅声明四个旧字段，未展示 `snapshot_version`、`refresh_due`、`model_count`、`last_failure_at_ms` 和 `last_failure_class`。

已实际运行：`npm run check` 通过，而 `node scripts/check-management-spa.mjs` 因权威契约不一致失败。仓库级门禁已能发现 drift；问题是前端局部检查只对照 vendored 副本，且生成器只生成操作/参数通道，未生成 schema 响应 DTO。仅重生成 client 不会自动完成页面接线。

**建议**：首批同步契约，更新相关 DTO/fixtures/渲染，验证字段语义与新状态；把权威契约一致性纳入前端日常检查入口。保留既有 12 个有意不接的操作，不以 100% 接线率为目标。

### F2 · P2 · 晚到响应可修改另一个配置版本的 revision，并允许 revision 倒退

**位置**：`web/prism/src/api/client.ts:118`、`web/prism/src/features/config-versions/versionStore.ts:37`、`web/prism/src/utils/revision.ts:29`。

请求发出时读取版本 A，但成功响应回来后 `advanceFromEtag` 使用当时的全局版本，未核对原请求属于谁。`advanceRevision` 也直接接受任何不同的 revision。

通过加载实际 TypeScript 模块、在内存中执行状态迁移复现：切换到 B 的 `rev-2` 后，A 的晚到 `rev-8` 把 B 改成 `rev-8`；随后晚到 `rev-3` 又让 B 倒退。无需真实网关即可复现。影响是错误的 If-Match 和编辑冲突；本次未证明后端会接受跨版本错误写入。

**建议**：响应必须绑定发出时的 config version/session generation，仅更新相同对象，且同版本 revision 单调推进；版本变化时取消可取消读取并关闭或重置旧表单。新增跨版本晚到和同版本乱序回归，而不是自动重放写请求。

### F3 · P2 · 鉴权失效只清版本，没有真正锁定会话

**位置**：`web/prism/src/api/client.ts:111`、`web/prism/src/session/sessionStore.ts:13`、`web/prism/src/app/AppShell.tsx:78`。

`management_access_denied` 被映射为 `session_invalid`，但 API 层只调用 `version.reset()`；没有调用 session `lock()`。全站也没有其他失效订阅完成该动作。

实际模块加合成 404 响应的内存复现结果：`unlocked=true`、Key 和 CSRF 仍在内存、version 已清除。AppShell 依赖 `unlocked`，因此不会按设计返回解锁页。后端仍拒绝请求，不是后端鉴权绕过。

**建议**：集中处理 session invalid：锁定、清除敏感会话/查询缓存、停止轮询、废弃旧 session 的晚到响应，并返回解锁。区分普通资源 404，不能全部按失效处理。

### 前端产品缺口，不应伪装成现有 API

1. **有效模型清单**：`listPublicModels` 是版本化配置，不是当前 Client Key 的授权后 `/v1/models`。管理 listener 和数据 listener 分离，Prism 的 CSP 是同源、生成客户端只接受 `/admin/*`。不能直接从浏览器跨端口调用 `/v1/models`，也不能以 Management Key 代替 Client Key。需要后端提供安全的管理投影，使用同一份 serving snapshot 和授权逻辑。
2. **完整配置图**：目前没有 listRoutes、候选枚举/修改/删除、alias 枚举等操作。`RouteWorkbench.tsx` 已明确写出这些限制；运营 inventory 的 `route_ids` 只能作为不完整提示，尤其漏掉尚未连好的草稿。应提出后端 CR，不能靠前端缓存补齐“完整图”。
3. **分析数据**：没有服务端时间桶、完整请求成败/延迟清单，账本与失败不是可相加的成功/失败全集。CPAMP 的趋势、请求成功率和 P95 需要后端统计口径与新增契约；当前可以改善已有视图，不能制造这些数字。
4. **界面组织**：运行时单页 1,711 行，承载多个不同 scope 的查询和操作。接口名、契约历史和实现解释大量占据首屏；建议迁入可展开的证据说明，把影响决策的时间范围、状态、来源与下一步保留在主视图。
5. **测试样本**：当前 fixture 仍有固定在 7/8 月的时间戳，本次页面查看出现“Fresh + 44 天前”。这是演示数据和时钟不一致，不是生产目录状态证据。开发演示用相对时间，测试固定时钟，并增加当前 contract 的状态组合。
6. **i18n**：骨架/枚举英文已做，正文英文仍是明确未排期事项。此次计划默认中文优先，不把已有暂缓项悄悄当成必须完成的重写。

## 5. 参考项目的借鉴边界

本次读取了 CPAMP 当前公开源码 `1ae656c82990c480f3f104326a08c6e0001eeb4c`，并查看公开 demo 的仪表盘和账号页。借鉴的是任务组织和信息层次，不照搬后端协议或静态样例数据。

| CPAMP 模式 | Prism 可采用 | 前提 |
|---|---|---|
| 凭证列表、平台筛选、详情入口 | 账号工作台；详情里分区显示权益与证据 | 复用现有 account pool/inventory；不合成 overall health。 |
| 仪表盘概览、失败入口、来源状态 | 少量关键指标和直接诊断入口 | 各指标明确窗口/范围/置信度；不能把累计值写成今日值。 |
| 请求筛选、详情、错误上下文 | 保留筛选条件，串起 request → attempts → account/route | 仅用现有安全字段；没有的延迟和响应体不补造。 |
| 账号 quota 可视化与巡检 | 后续独立能力 | CPAR 未提供等价 quota 数值、巡检调度或批量恢复契约。 |

参考：[项目说明](https://github.com/seakee/CPA-Manager-Plus)、[路由结构](https://github.com/seakee/CPA-Manager-Plus/blob/1ae656c82990c480f3f104326a08c6e0001eeb4c/apps/web/src/router/MainRoutes.tsx)、[账号页](https://github.com/seakee/CPA-Manager-Plus/blob/1ae656c82990c480f3f104326a08c6e0001eeb4c/apps/web/src/pages/AccountsPage.tsx)、[公开演示](https://seakee.github.io/CPA-Manager-Plus/#/demo)。演示中的数字是其虚构数据，不是本项目运行数据。

## 6. 本次验证与限制

| 验证 | 结果 |
|---|---|
| `ruby scripts/check-crate-boundaries.rb` | 21 个 workspace package，通过。 |
| `cargo test --locked -p gateway-control --lib management_operations_service` | 7/7。 |
| `cargo test --locked -p gateway-catalog --lib durable` | 4/4。 |
| `cargo test --locked -p gateway-control --lib billing_materializer` | 2/2。 |
| `cargo test --locked -p gateway-router --lib protocol_transform` | 26/26，包含近期工具/文本历史重放回归。 |
| `cargo test --locked -p gateway-stream --lib` | 14/14。 |
| `cargo test --locked -p gateway-auth --lib` | 11/11。 |
| `cargo test --locked -p gateway-upstream --lib upstream_client` | 12/12，受控 loopback transport。 |
| Provider pool 503 HTTP 定向回归 | 1/1；旧 handoff 的 500/503 问题已修复，本次不重新列为待办。 |
| Prism `type-check`、`test`、`check` | 通过；20 个测试文件、229 项单测。 |
| 仓库级 `node scripts/check-management-spa.mjs` | **失败**：权威契约与 vendored 副本不一致。 |
| F2/F3 内存复现 | 均复现；使用实际模块，虚构标识和合成响应，没有真实凭据。 |
| 前端构建产物 | HTTP 测试的构建链重新构建 SPA；4 个文件，共 663,932 字节（未压缩）。 |
| 页面查看 | 本地 fixture 的解锁、总览、运行时；CPAMP 公开 demo 的总览、账号页。 |

后端定向测试合计 **77 项通过**。没有执行全 workspace 测试、完整 E2E、生产端到端、真实上游探测、容量压测或新的正式 Delivery Gate；因此不据本报告宣称全项目测试通过或可发布。已有库测试通过不能覆盖本次发现的运行装配缺口。

推荐实施顺序：前端 F1/F2/F3 与后端 B1/B2 先行；随后处理目录失效/TTL 维护，交付账号与目录工作台；完整模型目录、路由编辑和分析升级按新增契约分批推进。
