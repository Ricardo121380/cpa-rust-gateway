# Prism 配置运行时应用：基础批次

本批修复“发布只换路由快照，实际执行器仍绑定启动配置”的问题，是
[CPA 功能对齐](../handoffs/prism-cpa-alignment-execution-20260912.md) 的后端基础。
前端简化流程、原生账号修改后的同步、完整联调和部署仍在实施。本批没有修改生产环境。

## 行为

`SnapshotPublicationService` 可连接运行装配器，在数据库激活前构建完整代际。
HTTP 请求入口捕获一次状态；客户端鉴权、执行器、账号池、出口状态、模型目录与管理投影使用同一代际。
旧请求持有旧代际，新的请求读取新代际。空配置可首次发布；无法服务的配置不能替换旧配置。

激活事务同时检查配置 revision、原 active、各凭据 revision 和原生账号库存时钟，并写审计。
OAuth 刷新不改变图 revision，因此单独检查凭据；旧草稿或回滚目标携带更旧凭据时拒绝发布。
冲突需要重读核对，不自动重放修改。普通与原生账号的并发名额按稳定绑定共享，旧代际尚未结束
的 lease 不会因切换而被清零。恢复持久额度时不覆盖更新的运行观测或同时间的恢复探测。

发布与正在进行的凭据刷新/目录请求串行，后台任务在有限的一次操作之间让出发布机会。
提交后停用旧任务，切换受管理的刷新和目录 worker；关闭任务监督器会取消其子任务。
事件队列、持久写入器、观测计数和 stored responses 由进程共用，不因换配置丢弃。
管理生命周期的数据库和编译工作通过有界 blocking 池执行，不阻塞 Actix listener。

## 本次验证

- `cargo test -p gateway --bin gateway runtime::tests`：95 项通过，含现有 loopback 协议矩阵、故障转移与事件持久化。
- 新 HTTP 集成回归 `live_publication_switches_admission_and_retains_captured_http_generation`：
  空安装发布、实际 HMAC Key 的新请求准入改变、捕获旧状态仍可完成、无效配置拒绝、回滚恢复准入通过。
- `cargo test -p gateway-upstream retired_generations_hold_capacity --lib`：跨多个已退役代际仍占并发名额，通过。
- `cargo test -p gateway-store prepared_activation --lib`：2 项通过，覆盖图/active/原生时钟冲突、
  同图 revision 下的凭据旋转，以及阻止旧草稿恢复旧 OAuth 材料。
- `snapshot_publisher::tests` 6 项、`compatible_egress_runtime_compiler::tests` 6 项、
  quota 观测回归 1 项、`p10_07_management_lifecycle` 2 项、关闭后 event writer flush 回归 1 项通过。
- `cargo clippy -p gateway --all-targets -- -D warnings` 通过。

这些是本批实际运行结果，不是历史测试总数。实际部署应用的浏览器任务链、原生修改同步和最终
仓库门禁仍在后续批次执行，不能用本报告替代整项功能交付与生产验收。
