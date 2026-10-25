# #68 / #72 / #74 本地集成验收

日期：2026-10-25（Asia/Shanghai）。状态：**PASS_LOCAL_INTEGRATION**。
分支：`feature/parallel-68-72-74-integration`；工作区：
`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar-parallel-68-72-74-integration-20261025`。

本轮在同一集成源码上重新执行验收，不继承旧分支 PASS。共同祖先仍为
`444558a8de930f5f1febd5a40ec5bdbbc30a15fb`；它不是此次新基线。
本报告的提交为受测源码之后的纯文档后继；最终可供下一批分支使用的完整 SHA 另写入容器的
`maintenance/2026-10-25-parallel-implementation-audit/INTEGRATION-RESULT.md`，避免自指提交身份。

## 来源和合并

| 来源 | 冻结 SHA | 纳入方式 |
|---|---|---|
| #68 | `3bf4ffef6badcb172f64784d4d16b4b24702dfc6` | merge `970d3d43`，完整历史可达 |
| #72 | `60073e72e8abd22f533178f6ec1b9fe97ce36c4c` | 独立集成分支起点 |
| #74 | `709aa4111726fb0c8efcc2f02ee740cdf6b5d60b` | 经安全后继完整纳入 |
| #74 安全后继 | `d78f44c76c16c4b18b740646aa0cb9389e04136c` | merge `c30e9491`，含安全代码 `30f3c8d1` |

四个来源 SHA 都是集成 HEAD 的祖先。原三个功能 worktree、安全后继 worktree、dirty 主克隆和
`cpar62-client-metadata` 均保留；没有 reset、clean、stash 或覆盖原工作区。
Codex worktree 工具因容器目录不是 Git 根返回错误，随后按交接允许的备用方式从真实仓库创建。

- 锁文件按冲突段合并，未整文件选边覆盖；包身份及规范化依赖边恰为三个安全后继的并集：
  **378 packages / 1148 edges**。保留 tokenizer `tiktoken-rs 0.12.0`、control 的直接 serde 边，
  统一 p256 0.14.0 / elliptic-curve 0.14.2 / crypto-bigint 0.7.8，移除旧直接 rand_core 边。
- `generate_key` 保留 #72 的 `Result` / `Randomness`，Console bootstrap、refresh 和 native usage
  三个生产调用点继续映射既有错误，不恢复隐式 panic。保留独有 JWK 固定坐标、无 padding、四字段、
  独立公钥重建/指纹、RFC6979、jti 长度、合法 JSON 篡改、尾随字节篡改、过期及缓存回归。
  #74 单独错钥测试与 #68 错钥/过期测试的 KeyMismatch 断言等价，合并后保留后者；两份 RFC 测试均保留。
- #72 继续使用 ADR-0128；#74 的 branch-local ADR-0128 改为 [ADR-0129](../adr/ADR-0129-account-quota-history.md)，
  记录原分支/SHA映射，Accepted 决策保持。历史报告仅修正其 ADR 链接，没有重标受测身份。
- 同名安全报告分别保存为 [#68 原文](crypto-bigint-security-fix-20261025-issue68.md) 和
  [#72 原文](crypto-bigint-security-fix-20261025-issue72.md)，与原提交逐字节一致；原路径成为索引。
- runtime/reload、deployment、management state/routes、OpenAPI、traceability、roadmap 的增量合并。
  计数 executor、五个 Usage 管理操作、quota-history 均在正式装配中；新增六操作、九 schemas 与来源并集一致。
  ADR 编号唯一；migration registry 为1–39各一次，0039唯一；#72没有新增migration。

## 受测源码和二进制

| 身份 | 值 |
|---|---|
| 受测代码提交 | `6a7ae73e79b57576ee37b3ace1d70d60f1909a9b` |
| 受测 Git tree | `2b0b9fab02c156a1f70311354c7742416d590a8f` |
| Cargo.lock SHA-256 | `a3950a764ae2dcae4e04a8a32ada596f33ea105dbfda8e9714b3e32d3216cd6d` |
| #72/#74 共用 gateway SHA-256 | `d66580b1c2464d7a94b6467a21fd88546fb0dd6d8e4eef3ba680f2816ab6a54e` |
| Full 默认 build metadata 的 gateway SHA-256 | `d1238b30750145615f09f4a04a9d6c9bb2756775e9efc9eab39331dc7da7203c` |
| 工具链 / 平台 | Rust 1.97.1；aarch64-apple-darwin；本地 dev profile |

`output/integration/source-02.json` 在运行前固定全部7150个受控文件哈希、commit/tree/lock及命令；
执行后逐项核对无变化。`test-binaries.json` 保存Full中113个实际测试可执行文件哈希。
`binary-manifest.json` 绑定构建命令、输入脚本/fixture、构建环境和固定二进制副本。
进程验收的构建显式设置 `GATEWAY_RELEASE_REVISION`、Rust版本和target；Full使用默认development
metadata，因此两份binary哈希不同，Rust源码相同。没有把运行时checkout HEAD单独当成来源证明。
固定进程验收binary保存于 `output/integration/bin/gateway-6a7ae73e79b5`。
所有构建使用此worktree独立target、`CARGO_BUILD_JOBS=2`，数据库/合成凭据与日志均隔离。
验收结束后，将本轮生成的`target-integration/`原样移至忽略目录`output/integration/target/`；
`target-relocation.json`记录前后binary哈希一致。历史构建路径保留在manifest/日志中，不改写。

## 本轮验证

| 项目 | 实际结果 |
|---|---|
| 项目 Full | **44/44 PASS，exit 0**，完成于2026-10-25 14:32:52 Asia/Shanghai；[完整步骤回执](parallel-integration-20261025-evidence/full.md) |
| Rust | **1695 passed / 0 failed / 21 ignored**，133个测试结果集合；ignored不计PASS |
| #68 生产 HTTP | 4/4：native/custom × API key/OAuth、模型/Alias、8个固定CPA oracle样例、权限/畸形/超限/上游故障、陈旧快照/取消；不产生推理事件、Usage、ledger或quota history |
| #72 正式进程 + TLS peer | **517/517**；7 calls / 7 receives；全过滤/全页/全桶/CSV/JSONL独立oracle、重试、失败、取消、缺价、跨Key、并发写入及重启失效；[回执](parallel-integration-20261025-evidence/usage-process.json) |
| #74 正式进程 + TLS peer | **20/20**＝保留原17项＋3项交叉检查；6次quota接收；两个实际进程均exit 0，重启读回相同历史；[回执](parallel-integration-20261025-evidence/quota-process.json) |
| schema38→39 | 完整38迁移前缀、真实Request/Attempt/Usage/价格/ledger夹具，正式store reopen升级；五张表逐SQLite单元保持，39注册一次且重迁移幂等；升级后HTTP快照/导出正常 |
| 价格版本/互不污染 | price-v1/v2在升级、ledger purge后的冻结明细/JSONL中保留；quota非空时Usage快照仍无行、费用null、JSONL空，导出/查询不增加quota或上游接收 |
| 取消容量修复 | `quota_history_cancelled_waiter_keeps_blocking_capacity`实际PASS，取消等待方不释放仍在阻塞的任务许可，任务结束恢复容量 |
| 供应链 | deny的advisories/bans/licenses/sources全通过；audit扫描378依赖，无漏洞或audit警告；未增加ignore、skip或降低门禁 |
| 静态门禁 | fmt、Clippy所有targets/features、源码/边界、文档/契约、全量tracked秘密扫描、whitespace均PASS |

schema38夹具通过仅撤销空的0039构造真实完整前缀，再由正式store打开迁移；不是只改版本号。
本轮没有运行历史schema38可执行文件或真实数据库升级，因此不声称旧版进程到新版进程升级资格。
#74的退出/新进程读取则是另行实际执行的进程证据。

独立命令与输入环境在`output/integration/process-commands.json`、`binary-manifest.json`中保存：

```sh
CARGO_TARGET_DIR="$PWD/target-integration" CARGO_BUILD_JOBS=2   TMPDIR="$PWD/output/integration/tmp" CHECK_REPORT_PATH="$PWD/output/integration/full-02.md" scripts/check.sh full
# 同一源码，显式嵌入来源身份后构建并固定副本
GATEWAY_RELEASE_REVISION=6a7ae73e79b57576ee37b3ace1d70d60f1909a9b   GATEWAY_RELEASE_RUST_VERSION=1.97.1 GATEWAY_RELEASE_TARGET=aarch64-apple-darwin   CARGO_TARGET_DIR="$PWD/target-integration" CARGO_BUILD_JOBS=2 cargo build --locked -p gateway --bin gateway
CPAR_USAGE_SNAPSHOT_ONLY=1 CPAR_GATEWAY_BINARY="$PWD/output/integration/bin/gateway-6a7ae73e79b5"   TMPDIR="$PWD/output/integration/tmp72" python3 scripts/acceptance/cpar-batch-b-http.py output/integration/process72-NEW
python3 scripts/acceptance/cpar74-quota-history.py --source "$PWD"   --binary "$PWD/output/integration/bin/gateway-6a7ae73e79b5" --output output/integration/process74-NEW
```

#72 的启动及重启均应用localhost-only出站围栏并关闭后台Provider任务；#74保留child-only
DNS/connect interposition、出站/文件访问围栏及外网拒绝探针。动态端口、独立临时状态、synthetic输入；
没有读取真实凭据或调用真实Provider。私有fixture状态不进入提交。

## Standards

固定评审范围`444558a8...196d58d7`。0项硬规范违规；1项P2资源生命周期问题和1项既有P3维护建议。
P2：quota capture/append阻塞任务在HTTP超时后可能继续运行，外层许可却释放。
`6a7ae73e`将普通/Kimi共用的4槽许可移入实际blocking任务，覆盖排队、Mutex及SQLite等待。
独立后续审查`196d58d7..6a7ae73e`未发现可行动问题，新增取消回归随最终Full实际通过。
P3：计数选择准备与推理部分重复，保留既有scheduler/lease原语，不借集成扩展路由重构。

## Spec

独立规格轴读取本轮GitHub #68/#72/#74正文、契约及完整集成差异，未发现可行动的遗漏、错误实现
或范围扩张。正式装配、Usage原ledger/价格复制、quota事务fence及DPoP三个错误调用点均核对。
静态审查不替代上述实际测试。[机器可读审查记录](parallel-integration-20261025-evidence/review.json)。

首轮各轴：Standards 2项（最高P2已修复，P3保留）；Spec 0项。修复复核无新增问题。

## 原失败和剩余边界

本轮首个Full在`196d58d7`的Clippy构建阶段因上述审查发现由主代理中断（exit143）；状态为
INTERRUPTED，不是PASS，也不伪称测试失败。`full-01.md`、`logs/full-01.log`和中断原因保留。
修复后从完整脚本重新执行Full，得到上述44/44。历史三分支供应链FAIL、测试数和源码身份均保留。

本轮deny仍有15组重复版本warning（block-buffer/chacha20/cpufeatures/crypto-common/digest/
getrandom/hmac/http/rand_core/sha2/shlex/socket2/syn/thiserror/thiserror-impl）。版本并集未引入
额外身份；分别来自既有AEAD、digest/P256、随机源、网络和宏依赖代际，保留原安全报告的逐项评估，
未用豁免压制。未知费用仍UNKNOWN，`cost_in_usd_ticks`保持NOT_ADMITTED。

真实Provider、实际Codex/Claude/Pi、固定Build服务、远端CI、release/OCI/Linux交付及部署均NOT_RUN；
不扩大旧#61–#67资格，也不声称首版READY。#75大容量/RSS/备份恢复压力资格未运行。
旧#64/#66制品专用harness的历史schema预期、Batch B非#72模式的原始计数预期未在本轮重验；
它们不属于此次517项结果，不可将本轮PASS套用到这些历史验收模式。

本轮只做本地集成、验证和提交，未push、评论/关闭工单、合入develop/main、部署或发起真实收费请求。
后续建议从最终文档后继SHA分别开始#69（请求控制/模型命名）、#73（价格预览/版本）、#75（容量）；
各自保留独立target/数据库和动态端口，不自动创建或启动下一批工作。

[汇总与证据哈希](parallel-integration-20261025-evidence/summary.json)提供原始日志/receipt的定位及摘要。
