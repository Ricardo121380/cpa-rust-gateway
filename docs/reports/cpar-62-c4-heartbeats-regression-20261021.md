# CPA-Rust #62 C4：Heartbeats / EOF / timeout 并发回归收尾

本轮 C4 回归收尾达到 **PASS_BOUNDED**：代码、专项回归及两路并发复核通过，最终完整 Full **45/45 PASS**。本轮证明了受控并发压力下的一种实际失败机制，并完成最小修复；原两次历史 Full 没有阶段轨迹，对它们逐次追溯为该机制仍属于有证据支持的推断。不得把本报告读作 #62、三个客户端或真实 Provider 验收完成。

## 基线、边界与保全

交付工作树为 `/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar62-client-metadata-20261020`，分支 `feature/62-client-metadata`；开工实际 HEAD 为 `189735ea3c281d770be6ad7ec159a5bfbdf7e2f8`，索引为空。C4 的 `50957d23`、StoredResponse 修正 `32ece7f4` 和最终报告 `189735ea` 用于定位，没有回退、reset、clean、stash 或整体暂存。

已亲读适用 AGENTS、CONTEXT、跨边界日志、ADR 索引、ADR-0012/0112/0116、BC-UPSTREAM-001、BC-PROTOCOL-010 及原 `scripts/check.sh`。本轮未修改 frontend/shared surfaces、契约、schema、C4 加密保存/授权 reader、确认期限或依赖。

18 项继承 dirty/untracked 输入逐项记录内容 SHA-256、mode、symlink、uid/gid，本地备份保留于本轮 output 目录。独立诊断与最终验证树均从交付树逐项复制并验证这些覆盖，没有仅 checkout commit 后宣称输入相同。原报告及其资产加原失败二进制共 42 个保全对象另行散列。完整备份与 inherited diff 只保留本地，不通过证据归档变相提交继承文档内容。

原失败二进制保持在：

`/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar62-client-metadata-20261020/eof-failure/final-full-gateway-failed`

SHA-256 为 `6b969388341b73776d04867e32924dda70631bdc509e0e5bfd4bc1bda9ab9f9b`。旧验证树的 7038 项原实际输入已与旧 `source-inputs-before.json.gz` 逐项比对，内容/mode/link 全部一致。没有以新结果覆盖旧二进制或日志。

GitHub 只读核对时 #62、#61 均 OPEN，#62 的原生 blocked_by 仍为 #61；origin 未列出 `feature/62-client-metadata`。没有 fetch 改写本地 refs、push、工单/依赖写入、合并、发布或部署。

## 动态复现与因果边界

本机为 Darwin 27 arm64、10 个逻辑 CPU，rustc/cargo 1.97.1、Node 26.8.2、npm 11.19.1。原 Full 记录的 `RUST_TEST_THREADS`、`RUSTFLAGS`、`CARGO_BUILD_JOBS`、`CARGO_PROFILE_TEST_OPT_LEVEL` 均未设置，本轮保持未设置；默认 libtest 并发，无全套串行化。旧材料未保存当时的 numeric worker count、完整继承环境或 OS 调度负载，不能称精确还原历史瞬时负载。

先在原验证 cwd、保留二进制和原输入上顺序做一次默认全 gateway harness 与一次 filtered 对照；之后才添加非敏感单调时间插桩。固定实验不是循环重试到绿：默认诊断一轮；两路并发压力下修复前一组、仅一个条件差分的修复后一组；最终无插桩候选再一组。每组均为两个完整 gateway harness 同时运行，每个 harness 保持默认并发。增加的第二个 harness 是明确记录的补充压力，不冒充旧 Full 的原始总负载。

| 运行 | 结果 | Heartbeats 冷构建与阶段 |
| --- | --- | --- |
| 原保留二进制，默认全 gateway | 270 PASS / 4 ignored，exit0，约103.7s | 无阶段探针；当前环境未重现 |
| 同一原二进制 filtered 对照 | 1 PASS，exit0，约1.69s | 不作为并发成功证据 |
| 仅阶段插桩，默认全 gateway | 270 PASS / 4 ignored | Responses 冷构建260.98ms，execute→headers约0.664ms |
| 插桩基线，并发压力第1路 | 269 PASS / 1 FAIL / 4 ignored | Responses 冷构建765.99ms，`total_exhausted_before_execute` |
| 插桩基线，并发压力第2路 | 267 PASS / 3 FAIL / 4 ignored | Responses589.52ms、Messages551.24ms，均在 execute 前耗尽total |
| 仅 HTTP native-root 条件差分，同样两路压力 | 每路270 PASS / 4 ignored | Responses冷构建0.064/0.116ms；Messages0.233/0.076ms |
| 无插桩transport候选3722c2c3，同样两路压力 | 每路270 PASS / 4 ignored，约95s | 新 Messages 断言同时执行通过，无阶段探针常量 |

两路修复前 Responses 都记录了同形 Heartbeats HTTP503/EgressUnavailable、独立完整请求接收0、实际 Attempt1、Request/Attempt durable commit2、writer failure/quarantine/pending0 和 lease0。该失败在 HTTP200 断言处终止，后续 SSE/ledger 断言不算这次 RED 已执行；接收0也不等于零网络字节、零副作用或零费用。

这两次动态轨迹明确是：`pool.send` 建立500ms total → cache miss → 同步 client build → 已超过500ms → `remaining_budget` 返回 EgressUnavailable；没有到达 execute 阶段。失败不是 writer 确认超时的同义描述。C4 的持久化确认发生在另外的边界，其期限和这项 transport total 分别保留。

一次有界1s栈采样在同组压力的其他真实 SocketTransport 调用中看到 `client_for → build_client → reqwest::ClientBuilder::build → rustls_native_certs::load_native_certs → macOS TrustSettings`。它不是失败瞬间 Heartbeats 的独立堆栈。结合安装依赖源码及只变更 native-root 条件的对照，证实明文 HTTP 冷路径在做不需要的原生信任库加载，该准备开销足以占满短 total。归因依靠阶段、采样及受控差分，不能仅因该测试请求未携带 C4 就声称 C4 的并发负载无关。

第二路补充压力中 Messages 原文同样为 `Error: "truncated SSE"`，其新阶段轨迹明确显示 Heartbeats 在 execute 前超时，status503；旧 Messages 测试没有先断言200，错误 JSON 被当作 SSE 解析。对此新增明确的 headers 断言。此轮另有一次 disconnect 用例失败，原始日志保留；未为它另立同因结论。原开发期那一次 Messages “truncated SSE” 缺少绑定到 case 的阶段证据，历史原因仍 UNKNOWN，不用本轮观察倒填。

## 首轮新 Full 暴露的独立 C4 夹具失败

本轮首个完整Full绑定3722c2c3，输入前后7079项完全一致。gateway为270 PASS/4 ignored，C4集成组却为4 PASS/1 FAIL/1 ignored；错误在`request_metadata_missing_writer_and_confirmation_timeout_fail_closed`中为`AlreadyExists / File exists`。Full仍是32 PASS + Rust tests FAIL + 后12 NOT_RUN，原日志及失败集成二进制均单独保留。同一失败二进制的一次默认/一次filtered对照通过，不能据此消除这条RED。

该函数进入队列/存储断言前，唯一会产生该文件系统错误的操作是`Directory::new`内的`create_dir`，目录名原来只由PID与SystemTime纳秒表示组成。本机Rust并发时钟探针80,000次读取仅有2,493个不同值，重复77,507次。将同一构造边界的时间参数固定，连续实际创建两个目录，第二次确定性重现同一AlreadyExists；补丁和二进制保留为独立RED。原失败现场没有记录同名目录的具体时间/归属，不能倒填它的精确同名来源；名称不能保证唯一以及实际碰撞机制已经得到动态证据。

只在C4测试夹具中加入与既有#59相同的AtomicU64进程内序号，保留PID和时间。不改生产存储、队列、确认期限或错误语义。固定相同时间戳的新回归现验证两个不同、mode0700、无数据库的目录；C4集成组为6 PASS/1 ignored child。这个新Full失败、确定性RED及修复后GREEN均保留。最终67db3304相对transport候选只改此夹具文件，transport和公共lifecycle源码哈希未变。

## 最小修改及关键契约

transport提交：`3722c2c3c6e660a0598d926bf2a02e40e74cecf7`；夹具提交及最终候选：`67db3304b901b5c8308c7b4fed99dd4e926e66ed`，tree `d3d747d521fd39d45e60a85516e44f7ab6200e8c`。累计仅3个本轮代码/测试文件。

1. `crates/gateway-upstream/src/upstream_client.rs`：Standard client 仅在 admitted scheme 为 HTTPS 时加载原生 TLS roots。HTTP 的 cache identity 已固定 scheme，且 redirect 禁止，不存在该 entry 从 HTTP 跳到 HTTPS 后需要 roots 的路径。HTTPS 保持原来的 native trust 设置；Chrome146、TLS 验证、代理/DNS pinning、重试和全部 deadline 起点/数值均未改。没有预热、移动total或建立新API。
2. 同文件新增冷 HTTP 回归：明确 cache 初始为空，idle200ms/total500ms，先200及部分body，持续字节流最终受原total截断，后续拉取仍失败。没有删除或放宽旧用例。
3. Messages EOF/idle/total 用例增加200及部分`cancel-63`输出断言、Hold/Heartbeats独立peer close、Request/Attempt各1、无Usage、唯一失败RequestFinished及first-content、空账本；原唯一error、无message_stop/message_delta、加密ConfirmedItems读取、接收1及lease0仍保留。

Responses 原用例保持成功headers和部分输出、恰好一次 `response.failed`、无 `response.completed`、不重发、接收1、Attempt1、Usage未知、空账本、peer close和lease释放。public侧仍是 **Actix test service**，上游是实际loopback TCP；这两项timeout用例没有因此变成public TCP listener验收。其他C4监听器测试的证据不能移借给它们。

已单独核对C4提交的 handoff 差异：SocketTransport 从固定content-type改成实际14个Build headers，是测试发送行为变化，不是“仅诊断”；生产handoff此前已经发送完整headers。本轮保留这项差异及C4对14个实际headers的隔离检查。复现轨迹在client build后、execute前已经失败；headers不参与该cache key。没有通过删除headers、请求字段、工具或能力制造成功。

补充首event阶段使用单独全新`target-stage-fresh`重建，记录8个Responses/Messages子场景均是首个解码ResponseStart先于public headers，之间约0.05–0.37ms；该filtered探针只证明阶段顺序。此前一次共享诊断target的补充构建遗漏upstream重编，完整日志与审计标记保留为PARTIAL，不用它绑定完整插桩源码。用于因果比较的前两组二进制在任何跨工作树target复用前已经固定；最终候选经探针常量检查为无插桩。

## 验证与独立审查

| 检查 | 结果 | 解释 |
| --- | --- | --- |
| upstream transport专项 | PASS：13 | 含新增cold500/200、connect/TTFB/idle/total、DNS/proxy、无隐藏retry |
| C4/Responses/Messages lifecycle | PASS：38，2 ignored | 含C4公共9项、pending receipts、工具历史、独立资源释放；固定客户端资格用例保持ignored |
| C4 persistence/reader/私有CLI | PASS：6，1 ignored child（原5项及新增fixture回归） | child由父用例实际启动；含真实writer kill/restart、owner/conflict/expiry/rotation/readback |
| 针对gateway/upstream Clippy | PASS | locked、all-targets、all-features、-D warnings |
| 无插桩候选并发 | PASS：两路各270 / 4 ignored | 完整gateway harness，每路默认并发 |
| 首轮新Full（3722c2c3） | FAIL | gateway270 PASS；C4夹具AlreadyExists，后12 NOT_RUN |
| 最终独立Full（67db3304） | **PASS，45/45，exit0** | gateway270 PASS/4 ignored；C4集成6 PASS/1 ignored child；全部步骤完整执行 |
| Standards | PASS静态，0可操作发现 | 独立静态审查，固定189735ea…→67db3304…及实际覆盖 |
| Spec | PASS静态，0可操作发现 | 与Standards分别评价，不继承Full或实际客户端资格 |

最终Full工作树：`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/validation/cpar62-c4-full-20261021-67db3304b901`。从候选detached并逐项应用18项原覆盖；使用该候选自己的Cargo.toml/Cargo.lock及全新独立`target-full-final`，运行未修改的`./scripts/check.sh full`。验证前后源码身份：**7079项前后完全相同，清单SHA-256均为`86c8d6c88eebac76d47cd84685c5b0966cd0bc3fc8328e85fa936dd478d7289e`**。这是18项overlay Full，不能称bare产品树Full。manifest/lock/toolchain均与开工值一致。

原两次Full继续保持 **32 PASS + Rust tests FAIL + 后续12 NOT_RUN**；后来单独12 PASS仍是另一条记录，未拼接成旧Full PASS。新Full以独立完整执行结果评价。

## 交付与后续范围

既有工作最终保全：**18/18内容、mode、symlink、uid/gid一致；继承tracked diff逐字节相同；42个历史报告/资产/原二进制对象全部不变**。本轮按两个代码提交逐次只暂存3个自有代码/测试文件；报告和脱敏证据另行增量提交，没有把18项既有文档和规则暂存到任何本轮提交。源码/原证据、二进制和索引的保全记录均可复核。

C4的字段契约、持久化确认与加密诊断能力保留。本轮对#62的贡献限定于这个并发回归切片。#62仍OPEN/BLOCKED，#61成功metadata/cost_in_usd_ticks仍NOT_ADMITTED；原样Codex的其他字段阻塞没有解除。C5 web_search、C6 input id、ParallelTools、namespace Build激活、Claude新字段及三个客户端/真实Provider资格均不在本轮实现范围。

真实Provider、真实账号/收费请求、真实客户端新增验收、远端CI、push、工单/依赖修改、合并、发布和部署均NOT_RUN。后续可回到既有字段/资格任务；不能据此关闭#62或扩大成功范围。

## 证据入口

本轮完整本地产物：`/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar62-c4-regression-20261021/`。其中保留对照/候选、本轮首Full失败的可执行副本、target、完整备份及本轮RED；最初6b969388…失败二进制仍在上文原路径。这些大文件和继承文档副本不提交。

- [代码候选与二进制绑定](assets/cpar62-c4-heartbeats-regression-20261021/candidate-final.json)、[最终源码绑定](assets/cpar62-c4-heartbeats-regression-20261021/source-binding.json)、[Full环境](assets/cpar62-c4-heartbeats-regression-20261021/final-full-environment.json)。
- [18项覆盖来源](assets/cpar62-c4-heartbeats-regression-20261021/validation-overlay-final-binding.json)、[保全结果](assets/cpar62-c4-heartbeats-regression-20261021/preservation-final.json)、[transport增量暂存](assets/cpar62-c4-heartbeats-regression-20261021/owned-code-staging.json)、[fixture增量暂存](assets/cpar62-c4-heartbeats-regression-20261021/owned-fixture-staging.json)。
- [修复前后对照](assets/cpar62-c4-heartbeats-regression-20261021/comparison.json)、[动态阶段归纳](assets/cpar62-c4-heartbeats-regression-20261021/diagnostic-stress-1-stages.json)、[第二路阶段](assets/cpar62-c4-heartbeats-regression-20261021/diagnostic-stress-2-stages.json)、[首event完整阶段](assets/cpar62-c4-heartbeats-regression-20261021/first-event-fresh-binding.json)。
- [C4夹具独立RED/GREEN](assets/cpar62-c4-heartbeats-regression-20261021/c4-fixture-comparison.json)、[本轮首Full失败](assets/cpar62-c4-heartbeats-regression-20261021/first-full-check.md)。
- [最终完整Full检查](assets/cpar62-c4-heartbeats-regression-20261021/final-full-v2-check.md)、[Full原始日志](assets/cpar62-c4-heartbeats-regression-20261021/final-full-v2.log.gz)、[独立Standards/Spec](assets/cpar62-c4-heartbeats-regression-20261021/review-final.md)。
- [显式allowlist证据包](assets/cpar62-c4-heartbeats-regression-20261021/development-evidence.tar.gz)、[成员清单](assets/cpar62-c4-heartbeats-regression-20261021/archive-members.json)、[内容/路径扫描](assets/cpar62-c4-heartbeats-regression-20261021/artifact-scan.json)。原报告及两次失败Full原资产路径完全保留。

证据归档采用显式allowlist，117个普通文件成员逐项核对bytes/mode/SHA-256并读取内容扫描，无DB、key、客户端配置、target或可执行文件。源码清单内同一个历史tracked任务文件名在4份清单中触发模式；按实际整行hash、文件名hash、原Git blob及文件hash验证后，仅接受这4条精确例外，见[扫描例外](assets/cpar62-c4-heartbeats-regression-20261021/artifact-scan-exceptions.json)。首次扫描阻断及核验记录也在归档内，原输入清单与扫描regex均未修改。

报告/资产提交仅增加本报告与本轮资产；Full绑定的是上述最终代码候选67db3304及其18项实际覆盖。报告归档后以文档链接、内容/路径扫描、Git diff/whitespace检查和仅自有路径暂存收尾，不借报告commit的不同SHA冒充重新执行Full。
