# #62：公开监听器对齐与原版 Pi 取消链重新验收

日期：2026-10-19（Asia/Shanghai）。**本轮受控取消链切片 PASS；#62 整票 OPEN / BLOCKED；真实 Grok NOT_RUN / 未证明可用。**
本轮交付为测试配置修复及重新取证，没有修改生产代码。#61 cost 继续 NOT_ADMITTED，正式依赖与原验收条件不变。

## 基线与适用规则

在产品根目录 `/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway` 继续
`feature/62-codex-responses-compat`。开工本地与远端均为 `36e937ff37f78be9da80e06ebb79ce3a1e33a1ba`；
`develop` 为 `c39bc07b1d53f831199f21cc5ae5062c987670b2`，保留上轮 `85b372bd` 的全部功能。
实际暂存区为空；9 个 tracked 修改、81 个 untracked 文件分别冻结哈希和模式，全部保留。
没有 reset、clean、stash、全量 add、历史回退或其他项目修改。

已读实际 AGENTS、cross-boundary-log、branch/layout/domain/issue 规则、CONTEXT、ADR-0100/0112/0113、
BC-PROTOCOL-009、两份 #62 报告及相应取消/存活/CLI/source 回执，并读取 #58/#61/#62 当前正文、全部评论和原生依赖。
其中 16 份原有本地文档仅作为验证覆盖层，逐项列入 source manifest，不属于本轮交付提交。
#62 唯一 blocker 仍为 #61，#61 的 #59/#60 blocker 均已关闭；未改变任何依赖边。
本轮没有触及前端及跨边界共享路径，不需要新增 cross-boundary 条目。

## 实际改动与配置核对

在以下现有测试公开 listener 分别增加 `.h1_allow_half_closed(false)`：

- `apps/gateway/src/runtime/batch_c/grok/standard_responses/lifecycle.rs` 的 `public_socket`；
- `apps/gateway/src/runtime/batch_c/grok/standard_responses/lifecycle/cli.rs` 的 `actual_cli_public_build_qualification`。

与 `apps/gateway/src/deployment.rs::run_servers` 的既有公开推理策略一致：HTTP/1 客户端 FIN 取消 handler/body，
包括等待上游响应头和已经开始但空闲的 SSE。客户端等候结果时须保留两半连接。management listener 策略保持。
这里只对齐取消策略，不声称测试 listener 的全部部署参数完全相同；生产连接上限仍独立保留。

Cargo.lock 为 actix-web 4.14.0 / actix-http 3.13.1。两条测试路径原来采用 half-close=true 默认值；
其余相关默认值与生产相同：HTTP connection keep-alive 5s、首个请求头读取 timeout 5s、连接 shutdown timeout 1s。
HTTP keep-alive 管理请求之间的连接，与活动 SSE 的 15s byte-idle keepalive 不同；请求头 timeout 也不是上游推理 deadline。
本轮没有缩短任何 timeout、改变 15s SSE keepalive、增加服务框架或改变管理端策略。

新增公开 socket 行为验证复用 production compiler/factory、真实 leased Build adapter、受控 socket handoff、
UpstreamClientPool、独立 TCP 上游及实际 writer：正常两半保持打开时完成；等待响应头/空闲 SSE × FIN/RST 四变体；
FIN 保留客户端读半边直到观察到上游 close 和 lease=0。既有挂起 Usage 确认测试也扩展为 standard/rejected Usage × FIN/RST。
没有以固定 sleep 代替资源条件；一秒 helper 是既有诊断界限，不是新产品 SLA。

## 15 秒现象：动态对照及限制

新增 FIN/header 用例在旧配置下先 RED：一秒时真实 lease=1、独立 peer_close=0。
最初的仪器化直接连接对照只变更两行 listener 配置，源文件逐字节差异在 listener-only binding 中核对。
随后建立不可变对照：最终验证源码 `0d8e548a5e983350e8d890d296405da24d669ffd` 与旧默认值控制源码
`8fb45692dca5231e371eda6d592a78026e6c841a` **仅差上述两行**，其余代码、manifest/lock、文档覆盖层相同。

| 实际原版 Pi 场景 | abort 发出至释放观测上界 | 一秒探针 | 同 PID 存活至观测结束 | 独立 close / 实际 lease | 结果 |
| --- | --- | --- | --- | --- | --- |
| 旧默认值，不可变直接连接 | 14,991ms | FAIL | PID 71094：是 | 1 / 0，均在清理前 | 旧诊断 FAIL 保留 |
| 对齐后，最终直接连接 | 14ms | PASS | PID 74531：是 | 1 / 0，均在清理前 | 受控取消 PASS |
| 对齐后，严格 Codex 门禁运行中的直接 Pi | 5ms | PASS | PID 74855：是 | 1 / 0，均在清理前 | Pi PASS；Codex 门禁仍 FAIL |
| 对齐后，额外 TCP 转发观察 | 12ms | PASS | PID 74736：是 | 1 / 0，均在清理前 | 仅诊断拓扑；不替代直接连接 |

直接连接的旧配置响应末次块包含原有 15s SSE keepalive，对齐后在该 tick 前已关闭且没有 keepalive。
结合仅两行变化的直接对照，已在**该受控测试路径**证明旧监听器策略造成取消后的保留；旧约 15s 延迟不能当作生产表现。
Actix 本地源码解释了 read-half EOF 在允许 half-close 时保留活动响应的机制。没有抓到内核层完整 FIN/RST/write-error 序列，
不将更细的内核触发顺序写成实测结论，也不外推生产网络时延。

当前账号无权打开 `/dev/bpf0` 抓取 lo0。可选 `--observe-client-tcp` 使用只转发字节的诊断 hop，记录原版 Pi 初始
`client_to_gateway read=EOF`，与 RST read error 区分；它不能证明 Pi 本地读半边状态，也不排除随后 RST。
该 hop 在 EOF 后关闭自身连接，会改变拓扑：早期旧配置观察在原有约 21s 窗口内未释放，明确 FAIL 并保留，不能作为直接连接结果。
早期该失败回执的 `release_observed_at_ms` 实为观察结束时间，正向字段为 false；最终源码对未观察到释放时输出 null。
没有扩大窗口后改称通过。

## 实际取消顺序、计数与回归

最终直接连接回执按毫秒绑定实际 text_delta、Rust 观察 lease=1、写出真实 RPC abort、收到 aborted、
独立上游 close/lease=0、同 PID 存活证明、最后才开始终止 CLI。观测时间为上界而非网络包精确延迟；
`total_observation_ms=0` 表示 RPC ack 后开始检查时资源已经释放，不表示 abort 零耗时。
全部实际 CLI 使用原版 Pi 0.84.3、其实际私有 SDK/OpenAI 依赖和 Node v26.8.2；安装树的内容、模式和链接前后不变。
隔离 HOME、配置、凭据、历史及工具目录，继承同一 loopback-only Seatbelt；真实 IPv4/IPv6 外网、真实用户目录读取和临时区外写入探针均 EPERM。

每次实际 Pi 成功场景保持回答→read(41)→相关 bash 计算/写入/读回(42)→回答→后续问答，共 5 个独立上游接收，
5 条 exact Attempt/Usage/ledger；已知 Usage 20/7、summary 与工具 ID/完整参数/结果历史、增量及唯一终态保持。
独立 background 负例仅增加 failed Request，receive/Attempt/Usage/ledger 增量均 0，不混入五次 CLI 推理。
失败场景 receive=1，唯一失败终态与 durable Usage/ledger 20/7；Pi error 的零用量显示仍单独保留为限制。
取消 receive/Attempt=1，Usage/ledger=0 行，唯一 RequestFinished=Cancelled，无完成/失败伪终态、重发或补证读取；
无 terminal Usage 不推出零费用。现有 ledger oracle 继续核对 exact Request/Attempt/owner/source-event 的唯一关联，cost UNKNOWN。
既有生命周期显式后续公共请求继续证明同一容量可复用；不把 CLI kill 或 server.stop 计作取消释放。

提前退出反例的原始 red/green 日志重新点验并保留；当前驱动的 observation 期间退出检查、结束时同 PID 检查和 Rust 核对没有放宽。
Codex 0.161.0 原请求继续按现有契约处理 namespace、client_metadata、web_search 等控制，原样请求 HTTP400、独立 receive/Attempt/Usage/ledger=0。
`--require-codex-success` 六项仍 false；最终 preparation 子进程 exit0，而严格外层 exit1，准确拒绝兼容声明。
未重复字段研究、删请求字段或改变成功判据。

## 验证源码、Full 与历史失败

代码提交 `ffedb1fa947fd46ee582c02c76f0eda6648372fc`，与评审候选 `ab6c88a02e865db5317ac27894f6ad2ee67a40df` 的 tree 完全相同：`f80e070b6e16423e7d32dbc528a59de37815eb73`。
最终验证源码 `0d8e548a5e983350e8d890d296405da24d669ffd`，tree `fb60a51920a6bb43626f6c1e5da8ecc57c456b33`，
位于 `/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/validation/cpar62-listener-cancel-20261019-ab6c88a02e86`。
五份本轮源码及四份 manifest/lock 的字节哈希与产品 checkout 一致；16 份验证文档覆盖层单列。
这不是纯交付提交树 Full，覆盖层不进入功能分支提交。

定向 Grok **35 passed / 0 failed / 2 ignored**；类型检查、严格 Clippy、格式检查 PASS。
最终同一源码 **Full 45/45 PASS；Rust 1553 passed / 0 failed / 14 ignored；534.643s**，包括 summary-only JSON/SSE/工具历史、
#59/#60/#61、共享 HTTP streaming、Usage/租约、serve/agent、文档/秘密扫描、cargo deny 与 audit。
实际 CLI opt-in 另行执行，上表和 raw qualification/driver/accounting 回执为其证据，不能由 Full ignored 代替。

保留的新验证反馈包括 timestamp i64/u64 编译错误和 Duration Clippy 提示，不记为生产缺陷。
切换不可变工作树时，共享 target 复用了内嵌旧 CARGO_MANIFEST_DIR 的 gateway 测试二进制；隔离边界拒绝打开旧驱动，CLI 未启动，
该次 FAIL 不进入验收。只更新最终验证工作树 main.rs 的 mtime 强制重编 gateway，源码字节/manifest/lock/隔离规则完全不变；
新输出目录回放通过后，核对源绑定和验证 checkout 的干净状态。该构建缓存反馈及原始失败也保留。
本轮没有发现对齐后仍需生产修复的缺陷；deployment、adapter、共享 pump、数据库/schema、协议及依赖均未修改。

## 评审、交付与剩余项

Standards/Spec 独立最终评审固定比较 `36e937ff` → `ab6c88a0`。Standards **PASS：0 硬违规、0 Fowler 判断项**；另有 1 项非阻断 P3 静态观察：可选 relay 的 OSError 支路未细分 connect/recv/sendall，却统一使用 read 字段。当前空 recv 的 EOF 分支不受影响；不宣称异常支路能归因 RST 读操作，保留为诊断限制。Spec **本轮切片 PASS，整票 PARTIAL/BLOCKED**：0 新增行为 bug、0 scope creep、0 已核对证据虚报，3 类整票缺口保留。
本轮只提交上述五份测试/驱动源文件以及本报告/脱敏回执。原 90 个 dirty 项内容和权限不变。
发布前扫描每个新增 blob 版本、明文回执、压缩归档及解压成员，并逐成员独立读回 SHA256；不发布凭据目录、SQLite 或 raw ESM stderr。
分支正常推送及唯一 #62 评论发生在这些审查与检查之后，Git/GitHub API 读回另存本地 delivery receipt；不代表远端 CI、发布或部署。

#62 整票仍缺实际 Codex 必需控制与工具闭环/native 生命周期资格、#61 成功完整元数据加密留存/确认/授权读回，
以及 Pi 失败用量显示和未验证模式等独立缺口。固定 Build 服务资格、真实模型关系、真实 Grok 与其他客户端发行形态仍需各自证据。
#58/#61/#62 保持 OPEN，不推进 #63–#67；没有真实 Provider/账号/新增预算、部署、发布、tags、main/develop 变更。

可审查回执：[验证汇总](assets/cpar-62-listener-cancel-20261019/verification-summary.json)、[源码与覆盖层](assets/cpar-62-listener-cancel-20261019/source-manifest.json)、[不可变旧配置对照](assets/cpar-62-listener-cancel-20261019/old-control-source.json)、[实际 CLI 时序与计数](assets/cpar-62-listener-cancel-20261019/final-client-runs.json)、[独立双轴评审](assets/cpar-62-listener-cancel-20261019/review-final.json)、[保留核对](assets/cpar-62-listener-cancel-20261019/preservation-final.json)、[逐成员哈希](assets/cpar-62-listener-cancel-20261019/evidence-manifest.json)、[选定原始回执归档](assets/cpar-62-listener-cancel-20261019/verified-evidence.tar.gz)。
