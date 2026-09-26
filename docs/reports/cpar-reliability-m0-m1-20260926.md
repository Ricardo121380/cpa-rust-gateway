# M0 / M1 可靠性实施与验收

日期：2026-09-26。范围是 [可靠性计划](../handoffs/cpar-reliability-alignment-plan-20260926.md) 的 M0/M1，非旧 Prism 同名阶段。实施基线 `213171acb7fdc295c3a258a1d0c79e2d93a73625`；M0 基线提交 `c751aff`；M1 实现提交 `98ce0d47263609e72f44104b16d9f6fbaeb6b893`。**M0 完成，M1 已实现并通过本地验收；M2–M4 未启动。** 本报告只证明本次执行的本地结果，生产保持 `b154396` / schema28，真实推理额度仍为 **0/12**。

## 实现范围

- **M0**：[操作/渠道映射、现网只读基线、性能/故障条件](../handoffs/cpar-reliability-m0-baseline-20260926.md) 已冻结。八工作区与解锁、八类现有渠道分别登记能力、真实接口、缺口和后续用例；CPA 没有的预算扩展不实施。生产只读核对版本、schema、事件/账本及账号/权限非秘密计数与指纹。
- **请求准入**：`GatewayEventSink::emit_confirmed` → 有界队列/oneshot → 现有后台 SQLite writer。生产 Request commit 后才进入 executor；满、关闭、写入不可用或2秒确认超时返回 `RecordingUnavailable`，上游不自动重放。SQLite 始终在 `spawn_blocking`，没有每个 SSE chunk 写入。
- **关联与终态**：router 的普通/固定/续接路径确认 Attempt 后才能输出或重试；请求内 scope 为 final Usage 附已确认的 Attempt ID，保留 Response ID，冲突关联被拒。JSON/SSE/WS/compact 共用观测；JSON 在实际 body handoff 后才有成功资格；首个 WS 源事件错误记失败。缺 Usage 仍未知，成功交付不等于完整或已计价的用量。
- **schema29**：新增 `gateway_request_recording`（active/finished/interrupted）和 `gateway_event_quarantine`；旧事件、账号、权限和账本不改写。启动把上次活动请求标 interrupted，既有历史查询仍可显示 unknown，不伪造终态/token/费用。毒记录持久保留指纹、分类和有界安全元数据后，才释放 pending；正常记录继续处理。
- **安全状态**：受管理鉴权保护的 metrics 新增接受新请求、记录系统状态、未确认数、最近 commit、确认失败和启动恢复未知数；既有计费处理状态仍单独展示。`healthz` 只表示进程存活。权威 OpenAPI 指标描述及安全错误枚举已同步，并运行 sync-contract；JSON 对象字段和前端源码均未改变。

## 保证及边界

生产总是使用已附 writer 的队列；测试/非生产 embedding 的 Noop 或仅入队实现不代表耐久保证。确认超时的写入可能随后提交，但调用方不会据此补发上游请求。

Request 已持久，而进程在 Attempt、Usage 或终态确认之前被强杀时，只能恢复已知事实。取消/Drop 不能等待异步确认，检查有界入队结果；若无法保存终态，Request 仍可追踪且保持未知。JSON sized body 可能在唯一 chunk 交付后直接被框架 Drop，此时检查终态入队而非提前伪造成功。服务器向 Actix 交付不保证远端客户端已收全。

旧事件没有新增字段时继续读取；生产生成的新事件携带明确关联。保留原有一请求最终 Usage 语义，本轮未扩展失败 attempt 的计费政策，也不引入硬预算。重启无法恢复未曾观测的 token，11 条旧歧义事件没有被“补账”。

## 定向证据

| 验收项 | 实际测试与结果 |
|---|---|
| M1-01 饱和/存储故障 | `required_admission_full_rejects_every_http_protocol_before_executor` 覆盖 Responses/Chat/Messages，503且 executor调用0；`unavailable_sqlite_writer_rejects_http_before_executor` 使用真实 SQLite 不可访问路径；`locked_database_fails_closed_then_recovers_without_replaying_upstream` 使用实际数据库写锁；已有 `sqlite_full_retains_one_finite_batch_until_capacity_returns` 注入 SQLite FULL |
| M1-02 进程故障 | `process_kill_at_each_confirmed_boundary_recovers_only_known_facts` 真正启动四个独立子进程，分别确认 Request/Attempt/Usage/Finished 后 kill；重开只见已确认记录，未完成为interrupted，重放不增行；不是仅drop内存对象 |
| M1-03 关联/幂等 | `out_of_order_explicit_lineage_and_state_are_idempotent`、`request_scope_attaches_confirmed_attempt_and_rejects_conflicting_usage`、`explicit_usage_attempt_does_not_follow_a_later_attempt`；旧Usage重放/迟到补齐与物化测试同时回归 |
| M1-04 隔离 | `acknowledgements_follow_commit_and_poison_survives_restart` 每次确认后查实际DB、重开后毒记录仍在；相同指纹只一行；既有原子写入与瞬时隔离故障回归保留 |
| M1-05 历史 | 手动执行 `historical_production_projection_survives_migration_and_partial_read`：schema28→29→28→29，2,356事件、699账本及六张源表值指纹不变；全历史严格读取仍拒绝歧义，部分读取699条，排除11条/4组 |
| M1-06 终态/租约/关停 | `json_drop_records_success_only_after_body_handoff`、`saturated_usage_queue_cannot_report_success`、`failed_attempt_recording_stops_retry_and_releases_lease`、`sse_end_can_be_polled_again_while_terminal_commit_is_pending`、原有SSE/WS/取消/租约及serve排空回归；runtime装配测试确认四类记录与显式Usage关联 |

历史验证输入仅为 owner-readable 的历史事件/计费元数据投影，不含认证库、凭据、请求正文；SHA256 `7227a8ac8fb9e138aeac4a5954baef4b562668efd32c534e94569b23fb3b80a6`。原投影不提交仓库。此项不是整个生产数据库的升级/回退发布演练；M4 仍须验证完整隔离副本。

## 门禁与耗时

- `./scripts/check.sh fast` 最终通过：[逐步骤回执](evidence/cpar-reliability-m0-m1-20260926/fast-gate.md)。本轮工作区 Rust **1,327 passed / 0 failed / 12 ignored**；包括 schema、契约、嵌入、升级回退和故障回归。12项忽略包含原有9项及本轮3项手动入口；其中 crash_child 由自动父测试启动4次，另外历史副本与耗时入口单独手动运行通过。
- `npm --prefix web/prism run type-check` 与 `npm --prefix web/prism run test` 通过：**54 个文件、407 项单测**。`node scripts/check-management-spa.mjs` 在 fast 内通过：152个管理操作、CSP/同源约束、四文件两次构建字节一致。没有前端行为改动，本轮未跑 EgoLite 视觉验收；M3/M4 保留该任务。
- fast 中真实 gateway + 自有 loopback TLS mock：12个 Agent 回合覆盖 JSON/SSE × 手动历史/存储续接 × 3轮；3个非法输入拒绝，12条成功请求物化账本。另5个请求覆盖成功2、失败2、取消1，2条成功账本；两页目录刷新不扩大开放/Key权限。**真实 Provider 推理为0**。[机器可读结果](evidence/cpar-reliability-m0-m1-20260926/verification.json)
- 独立历史副本入口通过：[回执](evidence/cpar-reliability-m0-m1-20260926/historical-projection.txt)。先前失败 fast 摘要也保留：[失败回执](evidence/cpar-reliability-m0-m1-20260926/fast-initial-failure.md)。

测量同一测试二进制、同机空闲构建期、真实 SQLite writer、进程内 Actix + 合成 executor。基线是**仅队列入队确认的旧语义对照**，不是旧发布二进制；每种先预热一次，再独立5次，计时覆盖请求处理和完整 body 读取，清空队列在计时外。[原始样本](evidence/cpar-reliability-m0-m1-20260926/mock-confirmation-timing.txt)

| 模式 | 5次中位数 | 与入队对照差值 |
|---|---:|---:|
| 仅入队确认 | 0.193292 ms | — |
| 持久 commit 确认 | 0.903833 ms | +0.710541 ms |

相对增加约368%，绝对增加不足1ms，未触发预先冻结的“同时超过10%和50ms”回归条件。此微基准只隔离记录确认成本，不能当作生产网络延迟、旧发布性能比较或SLA。M4 的10万/100万大样本尚未执行。

上述测试在提交前候选工作树上运行，实现源码对应 `98ce0d4`，不是该提交的签名发布产物。暂未制作/部署候选、未跑供应链发布门禁、未使用真实推理额度；这些仍属于 M4。

## 本轮发现并修复的门禁失败

首次 fast 的 Rust 测试通过，但其后真实 gateway + loopback TLS mock 的 Agent 多轮 SSE 发生 `IncompleteRead`。实际日志确认：终态异步 commit 返回 Pending 后，下一次 body poll 再次轮询已经结束的 `futures::stream::unfold`，触发 panic。已对内部流使用 fuse 保留结束状态；新增确定性延迟确认测试，不靠重试推理掩盖异常。原失败门禁摘要保留在证据目录，最终结果另列。

此前严格 Clippy 发现新增方法缺文档/属性、转换及测试表达问题，均修正；没有降低 lint。三个旧单测只为 Request/Usage 预留队列容量、忽略终态，现为完整事件数量提供容量并断言终态；独立容量1的饱和失败测试仍保留。生产队列上限未扩大。

## 接续工作

M2 开始前沿用本次基线：先补 Claude/Kiro 等既有渠道真实支持的刷新装配、失效分类与 CAS，再做同账号轮转后的续接隔离和完整渠道回归。M3 承接诊断/UI恢复；M4 承接大样本、完整生产副本、12次内真实验收与正式发布。

升级候选到 schema29 后不可只换回 schema28 旧二进制，因为旧解码拒绝新字段。未来回滚保留本次运行证据并使用已演练的升级前完整快照，不删除新历史伪装兼容。本次未做生产升级或数据回滚。
