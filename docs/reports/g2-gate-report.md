# G2 phase gate report

| 字段 | 值 |
|---|---|
| 计划 | `v1.1` |
| Gate | `G2` |
| 日期 | `2026-07-19` |
| 验证分支 | `codex/p2-10-management-api-cli` |
| 被测实现 Commit | `fccdc74575920eb3e2ed955cf0d8d1aee73cf570`；验证记录 `c569099a25f7299d7d996c20f0e1374687783115` |
| 本地结果 | `PASS` |
| 最终状态 | `PASS`（GitHub Fast + Full 门禁均通过） |

## 结论

P2-01 至 P2-10 的实现、任务级 review、本地 Fast/Full 门禁和任务级 GitHub 双门禁均已完成。
P2-10 增加了最小本地管理生命周期、事务绑定审计事件、启动时一键回滚前驱重建和 CLI，且其
GitHub Actions [29687760913](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29687760913)
的 Fast/Full 均为 PASS。

验证记录 `c569099` 的 GitHub Actions
[29688265117](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29688265117)
已完成：Fast gate 和 Full supply-chain gate 均为 PASS。因此 G2 的五项退出条件已满足，P2
正式完成。本报告不引入 P3 连接、HTTP 上游、Provider 运行时、代理、TLS 或真实流量；P3 仍为
`PENDING`，未由本 Gate 自动启动。

## G2 条件与证据

| 条件 | 当前证据 | 结果 |
|---|---|---|
| 无效 Alias、悬空 Candidate、重复 Endpoint Format 整版拒绝发布 | `gateway-control::route_compiler::tests::conflict_matrix_returns_stable_codes`，以及 P2-06 的 Alias、引用、能力与冲突矩阵；`SnapshotPublicationService` 在 SQLite 激活与 `ArcSwap` 前编译 | PASS |
| 100 个并发请求跨 Snapshot 发布仍固定使用各自起始版本 | `gateway-router::route_snapshot::tests::one_hundred_readers_retain_the_snapshot_loaded_before_publication`；每个读者持有自己的 `Arc<RouteSnapshot>` | PASS |
| 数据库和备份中不存在明文上游 Secret 或完整 Client Key | P2-03 AEAD envelope、P2-04 HMAC Client Key、P2-05 opaque Repository、`encrypted_credentials_remain_opaque_when_written`、redacted Debug 测试和 `secret-scan.sh --all` | PASS |
| 本机上游只有显式 Egress Allowlist 才能访问 | P2-09 `EgressPolicy` 测试覆盖默认私网拒绝、显式私网 CIDR 例外、DNS Rebinding 和发布前静态校验；无 Policy 的启用 Upstream 不能发布 | PASS |
| 推理热路径通过测试证明不调用 Repository | P2-07 `RouteSnapshotRegistry::load()` 的无锁 `Arc` 路径、P2-08 `SnapshotClientKeyAuthenticator` 单次 Snapshot 读取测试、crate-boundaries 检查均确认 Router/HTTP 不依赖 Store/Control 运行时查询 | PASS |

## P2-10 补充验收

- `ManagementService` 对完整 draft 创建执行“图写入 + `config_created` 审计”同一 SQLite
  事务；发布和回滚执行“Version 状态转换 + 正确审计动作”同一事务，随后才做不可失败的
  `ArcSwap` commit。
- `management_audit_events` 只存安全元数据，具有外键、动作/时间/actor 边界和 SQLite
  `UPDATE`/`DELETE` 拒绝触发器。它记录的确切被替换 Version 用于重启后的回滚前驱重建，不会
  从任意 archived Version 猜测回滚目标。
- P2-10 E2E 覆盖两个 draft 的创建、验证、发布、进程重启、回滚和五条有序审计事件；CLI
  实测同一流程。首个发布的 synthetic bootstrap Snapshot 永远不能作为数据库回滚目标。
- 本地 `./scripts/check.sh fast` 和 `./scripts/check.sh full` 均通过；Full 的第一次 RustSec
  advisory 拉取遇到外部 Git I/O 失败，未跳过检查，立即完整重跑后通过依赖政策和 RustSec audit。

## 已核验的任务实现和 CI

| Task | 实现 Commit | GitHub CI |
|---|---|---|
| P2-01 | `451b54a` | [29672840348](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29672840348) Fast + Full PASS |
| P2-02 | `a60bfe1` | [29673886566](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29673886566) Fast + Full PASS |
| P2-03 | `cd9d79b` | [29675091607](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29675091607) Fast + Full PASS |
| P2-04 | `0bff7f2` | [29676361709](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29676361709) Fast + Full PASS |
| P2-05 | `7ff1db1` | [29677865862](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29677865862) Fast + Full PASS |
| P2-06 | `58dbdb8` | [29679742048](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29679742048) Fast + Full PASS |
| P2-07 | `bb6a988` | [29681389853](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29681389853) Fast + Full PASS |
| P2-08 | `ee9a679` | [29683227429](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29683227429) Fast + Full PASS |
| P2-09 | `2be6ea7` | [29685685002](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29685685002) Fast + Full PASS; verification record [29686225793](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29686225793) Fast + Full PASS |
| P2-10 | `fccdc74` | [29687760913](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29687760913) Fast + Full PASS |

## Review

- 复核了开发计划的 P2/G2 条件、P2-01 至 P2-10 报告、ADR-0001 至 ADR-0010、契约、迁移
  顺序、crate boundary、Secret scan 和任务级 GitHub workflow 结论。
- 复核了 P2-10 的关键顺序：完整 Snapshot 构造和 registry reservation 在持久状态转换前；
  activation/audit 同事务；只有成功提交后才 `ArcSwap`。错误的 publish/rollback 审计动作会在
  activation 前被拒绝，失败不会留下部分状态。
- 复核确认推理热路径不引入 Repository：新 `ManagementService` 只在 control-plane/CLI 中，
  `gateway-router` 仍只保存/加载 immutable Snapshot，P3 文件、上游 Client 和 Provider 没有修改。
- 验证记录 `c569099` 的 GitHub Fast 与 Full 已完成且通过；本次将计划、报告目录和追踪索引
  同步为 P2/G2 `DONE`，不改变任何 P3 文件或运行时行为。

## 后续

P2 已结束，`P3-01` 保持 `PENDING`，不会由本 Gate 自动启动。任何 P3 启动仍必须遵守用户当前的
单 session 工作方式和“计划 → 实现 → review → Fast/Full → GitHub 验收”的顺序。
