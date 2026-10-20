# 隔离合成环境：诊断证据的进程升级与恢复

本说明只适用于 #65 的拥有式合成演练。它不授权恢复用户数据库、替换共享服务、调用真实 Provider 或发布。整票仍保留 #65 → #63 → #61 依赖；失败 Build Usage 和 Messages summary 的恢复不替代 #61 尚未准入的成功 Responses 元数据。`cost_in_usd_ticks` 仍 NOT_ADMITTED，缺失信息及费用 UNKNOWN。

## 执行输入

先按交付报告的 source-manifest 核对源码、Cargo manifest/lock、规则 overlay、构建日志和 binary/reader SHA。旧版固定为 `098c92a51ac549a38c8f15fd265e0fb379de2c77`（实际 schema35），当前为报告登记的 schema36 源码。各源码目录使用自己的 manifest/lock 和 target；不从其他目录挪用带旧 `CARGO_MANIFEST_DIR` 的测试程序。

构建依赖只使用已有 lock。分别在固定源码的 `web/prism` 执行 `npm ci --ignore-scripts --no-audit --no-fund`，再从各源码根目录使用独立 `CARGO_TARGET_DIR` 执行 `cargo build --locked -p gateway --bin gateway`。当前源码另外执行：

```sh
CARGO_TARGET_DIR=/absolute/owned/current-target cargo test --locked -p gateway \
  --test cpar65_process_support --no-run --message-format=json \
  > /absolute/owned/operator-build.jsonl
```

从 Cargo 的 `compiler-artifact` 记录选择 target.name 为 `cpar65_process_support` 的 executable。这个 ignored 测试是调用现有备份和队列 replay 的 operator helper，不是验收服务。它不随正常 Full 自动运行；验收主体始终是生产 `gateway serve`。

```sh
python3 /absolute/fixed-source/scripts/acceptance/cpar65-process-recovery.py \
  --current-binary /absolute/owned/current-target/debug/gateway \
  --historical-binary /absolute/owned/schema35-target/debug/gateway \
  --support-binary /absolute/owned/current-target/debug/deps/cpar65_process_support-HASH \
  --current-source /absolute/fixed-current-source \
  --historical-source /absolute/fixed-schema35-source \
  --output /absolute/owned/new-receipt-directory
```

输出目录必须不存在。驱动为每轮创建独立 0700 私有状态根，现场生成所有 key、凭据和 CA，并保留成功、失败、升级后历史与私有输出。`matrix.json` 登记该私有根；不得把整个私有根归档或提交。驱动 stdout 仅给结果和路径，reader 明文仅写 canonical 0700 目录中新建、独占 0600 文件。

## 进程与网络边界

每次 serve 显式传 `--state-dir`、`--credential-dir`、`--master-key-dir`、`--active-master-key-version`、两个拥有的 loopback listener 和 `--background-provider-tasks disabled`。readiness 同时验证管理鉴权与数据面鉴权；停止使用 SIGTERM、实际 wait、退出码 0、PID 已不存在和 WAL checkpoint。故障清理退出与场景内正常退出分开登记。

现有 `cpar-device-loopback.c` 只加载到拥有的 stock 子进程。固定 Build URL、生产 encoder、完整认证头、egress admission、DNS-pinned pool、TLS 主机名/SNI 验证和 decoder 均保持。测试 DNS/socket 最终指向拥有的 TLS peer；配置仅为合成 fixture 明确允许 `127.0.0.1/32`，使用合成 CA。Seatbelt 只允许该接收端口出站，IPv4/IPv6 文档地址与无关 loopback 负例由 OS 拒绝，另一个 receiver 接收数为零。不从环境开关推断隔离。

不可信 CA 与错误 hostname 在真实 serve 中分别拒绝；TCP accepts 和完整 HTTP receives 分开记录。此 interposition 不证明公网 DNS、真实 peer 与 pinned 地址相同、真实 Provider TLS/认证接受或 DNS-rebinding 防护资格。没有新增生产上游覆盖入口或关闭证书验证。

## 矩阵与故障归属

- B：历史 schema35 二进制先创建非空账号、停用意图、模型/权限、自定义连接/路由、真实公共请求、Usage/账本及无 companion 历史；新版生产 bootstrap 执行 35→36。禁止把新库 down migration 后称为历史状态。
- A：当前 schema36 正常停止、真实重新启动，比较完整列/行内容哈希并 exact-owner 授权读回两类证据。
- C：保留 key1，active 切 key2。旧 ciphertext 不 reseal，新写 key2；key2-only 只读新证据。外属为 UNKNOWN，缺/错 key、交换 ciphertext 认证失败。两类 SQL INSERT 故障都先返回 RecordingUnavailable，解除合成故障后只允许原 pending 事务落盘；失败交付仍失败，独立接收/Usage/账本均不重复。
- Expiry：直接 SQL 的 `(created,expires)=(0,1)` 仅是明确标注的保留时钟 fixture，不是公共请求生成的证据。生产事务清除过期 payload；summary 保留无内容 Attempt receipt，Native Usage 保留普通 source-event 屏障。同/异内容 replay 在重启、轮换及恢复后均不得重建或续期。
- 缺失、旧历史、未完成 SSE 和没有收到的 Usage 不补造；TLS 失败和畸形/截断请求也保留历史。ciphertext swap 只操作隔离副本。replay 通过 test-only helper 的真实现有 queue/writer 入口注入，不能冒充另一次公共请求生成。

## 一致加密恢复并采用恢复状态

1. 使用现有 `gateway-store::backup::create_encrypted_backup` 生成一致 SQLite snapshot 的 AEAD artifact；HTTP backup preflight 本身不导出 artifact。快照包括配置、事件、执行身份、证据、账本、recording 与必要 receipt。另存独立 Backup Key 和全部被引用 Master Key 版本。Backup Key 不能解开内部证据，保留它也不能替代 Master Keys。
2. 在新的隔离 restore-host 启动真实服务，用受保护 binary restore preflight/restore 提交 artifact。该 API 只生成此 host 的 `restore-target.sqlite3`，要求目标不存在；不会切换运行中的 `control.sqlite3`。
3. 停止并收割 restore-host。再创建独立空 serving-state，把已验证的 `restore-target.sqlite3` 作为其中新的 `control.sqlite3`；原 source、restore-host 和 target 全部保留。复制演练所需的外部 credentials、key ring 到新的私有目录。本演练没有覆盖既有数据库。
4. 真实 serve 显式使用新的 serving-state、credentials 和 key ring。核对进程、schema、两个 listener、公开模型/权限/停用意图，以及 exact owner/Request/Attempt 的授权读回。完整业务行、Usage/source/账本、过期 receipt 和 TTL 必须保持；允许的 quarantine、materializer/checkpoint 与 request-recording recovery 变化分别记录。
5. 在已恢复状态再提交同/异 expired replay，验证 receipt 未变、ciphertext 没有重建、没有 Provider 请求或重复计量。旧 reader 对不支持的 schema 准确拒绝；不能按版本号猜兼容。

admin-auth.sqlite3 属独立管理员认证存储，既有控制面备份不包含它；本合成驱动使用现场 Management Key，不演示迁移用户管理员认证。SQLite GC 只证明列级逻辑清除，不证明旧备份、WAL 或介质物理擦除。

失败时保留原始状态、新历史、日志及版本化密钥，不清空/重导入，也不以仅回退 binary、删除新表或 down migration 冒充安全恢复。真实环境的恢复需要另行授权和对应环境证据。
