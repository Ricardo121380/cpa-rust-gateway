# CPAR-66 当前源码 Linux ARM64 本地候选与 stock reader/serve（2026-10-22）

状态：**PASS_LOCAL_PREPARATION_BOUNDED**。本轮完成范围是当前输入的可核对 Linux ARM64 未签名候选，以及该交付 binary 的 bounded reader/serve/restart/exit 验证。#66 仍为 OPEN，#62/#64/#65 依赖保留；本报告不提供固定服务、三客户端新 Linux 运行、真实 Provider、远端 CI、签名或发布部署资格。

## 1. 开工输入、边界与实际基线

- 工作树：`/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar62-client-metadata-20261020`，分支 `feature/62-client-metadata`。开工实际 HEAD `550e3872f90c4be672ac45b9bda14b3839598276`，index 为空；18 项继承文件完整保留。用户给出的历史 SHA 仅用于定位。
- 最终源码候选：`737fd040e983bef329be27b5cf48b4f6986a5772`。交付证据 commit 在此之后，不能把 Markdown HEAD 改称 binary 受测源码。
- 实际 source 是 **base commit + overlay**：7,380 个输入；18 项 Git status overlay 加 15 项独立权限 overlay，共 33 项。15 项历史报告文件实际为 0600、Git tree 为 0644，不能仅从 Git status 推断完整权限。
- 亲读了根/工作树适用 AGENTS、CONTEXT、交付与质量规则、[D2 候选报告](cpar-64-d2-candidate-20261022.md)、ADR-0122、BC-PROTOCOL-016、[local controls](cpar-64-local-controls-20261022.md)、#66 正文/原生依赖，以及 P12、Linux、私密 reader/恢复契约。
- 使用现有 release builder、P12 manifest/verifier、SBOM generator、Dockerfile 与恢复 driver 的通用实际进程操作；没有运行归档旧 SSH/system epoch，没有打开 controlled-test-support 或模拟固定 Build 资格。
- 输入：冻结的 source archive/manifest/overlay、Cargo.lock、固定 Rust/Node、固定 ARM64 builder 与离线依赖缓存。产物：六文件 unsigned P12 set，外置 source/build/environment/run 回执。验收只允许新临时状态、合成凭据、三个显式 loopback 端口与本轮本地接收端。

GitHub 开工及最终只读快照均显示 #61/#62/#64/#65/#66/#67 为 OPEN；#66 `blocked_by=[62,64,65]`，`blocking=[67]`。未写 GitHub，未 push。

## 2. 最小实现与失败闭环

本轮代码增量只涉及 10 个文件：3 份既有集成测试入口、metadata fixture module、共享 fixture exporter，以及 `cpar66-source.py`、固定 builder recipe、`cpar66-build.sh`、Linux envelope、stock reader/serve driver。产品 Rust/协议/schema/SPA/锁文件、原 release 工作流和严格 verifier 均未改动。

- Source freezer 纳入所有实际 tracked/untracked 输入的 hash/type/mode/link/uid/gid；归档仅规范化 uid/gid 为 0，保留 mode/link。既有只 checkout ref 的 reproducibility 入口未被拿来冒充 overlay 构建。
- Fixture exporter 显式要求独占绝对路径与 0700，使用既有 writer/fixture、WAL checkpoint，导出 coherent DB、合成 key、私密 oracle。3 个 ignored exporter 已显式执行，不计作默认自动测试通过。
- Linux envelope 保留 macOS sandbox 的隔离目的：独占 `network=none` namespace；内核 firewall 只放行 19180/19181/19443 和已建立响应；拒绝其他 IPv4/全部 IPv6；readonly root/artifact/source；切换 65532，清空 capabilities/bounding set、启用 NoNewPrivs。没有用“容器启动成功”代替实际隔离检查。
- reader/serve driver 只使用交付 binary，验证 exe SHA、身份/schema、鉴权、私有输出、错误 owner/request/Attempt/key、真实新进程采用与退出。状态只在所有 cleanup check 后变为 PASS。

| 原始失败 / 负例 | 保留证据与修复结果 |
|---|---|
| 纯 Git archive 未包含当前 overlay | `red-ref-only-source.json`：18 项实际内容遗漏/不同；改用 base+overlay archive |
| 初次 permission preflight 发现 15 项 mode 丢失 | 原 Full/build 已中止，`aborted-full-input-preflight.json` 保留；修复 freezer 并在全新 target/input 上重跑 |
| P12 两文件 profile 不是完整制品 | `red-two-file-artifact.log` 真实拒绝，随后补齐六文件 set；未放宽 verifier |
| 初始 driver cleanup 可留下 PASS | 两轴审查共同发现 1 个 P2；`red-cleanup-status.json` 与 `green-cleanup-status.json` 故障注入闭环，实际 Linux cleanup 另证 |
| 初次 stock reader probe 错误要求 stdout 为空 | `red-reader-stdout.json` 与原 FAIL runtime 保留；既有 reader 有固定无值确认行。修正为精确成功/通用失败流白名单，仍禁止任何私密值 |
| registry/API/auth 准备失败 | 原日志保留；只修复本任务 VM proxy 与 per-call API，未更换依赖、架构或旧 binary |

所有负例仅操作本轮独占副本。先前已完成的 `1a8b6fc4` 制品及 FAIL/开发探测全部另存，不与最终候选混用。不同源码/revision 的两次完成构建不构成 reproducibility。

## 3. Source、构建与完整 unsigned set

实际输出根：`/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar66-local-candidate-20261022`。

| 对象 | SHA-256 / 身份 |
|---|---|
| final base commit | `737fd040e983bef329be27b5cf48b4f6986a5772` |
| source archive | `6117221bd1fe960b63adbcc33029b2fc91c82fe222ecd1eab1cc86df21be88d3` |
| actual input manifest | `bbd89a90c1e1deb470a0fd46bfd68196f89cc4d5325cbacef0cb4d728f65e9cd` |
| overlay manifest | `a33a04b2fbed604323e91e17b9630c8d229882f9bdf95fc9644c0fd72bd510c3` |
| Cargo.lock | `36564c0219707741d79ecb37d9fc4ee620ce2fe6801fdbe09b32fb4c8687c858` |
| SOURCE_DATE_EPOCH | `1792677294`，由本轮候选 commit 固定；未使用旧系统 epoch |
| builder image | `sha256:eca0fa3b1a319ee5ddaab9a3804af488536c51fe4125506cb1b8bc0039003aea` |
| binary | `17c1fd546d1a8fbb7d310bc3f62a872f23b9d1b5b3f8195e383fc9f86162bdf2` |
| artifact-manifest.json | `aff99e88cb24a7cf3f1489f0a712c3e37e83af76e29bb5768a9a552fcd255e0c` |

构建使用原生 `aarch64-unknown-linux-gnu`、Rust 1.97.1、Node 24.19.0、cargo-cyclonedx 0.5.9、`cargo build --release --locked --workspace`，默认 features；fresh input/target，`CARGO_BUILD_JOBS=2`、incremental=0、Cargo/npm offline、`--pull=never --network=none`。revision/Rust/target 显式注入。完整 `build-release.sh` 包括 Ruby normalization 正常退出，才提取 binary；不提取失败/中间 target。

`unsigned-artifact/` 精确包含以下六个 regular files；source、overlay 与补充回执位于外部，不污染 P12 精确文件集合：

| File | Bytes | SHA-256 |
|---|---:|---|
| `artifact-manifest.json` | 1016 | `aff99e88cb24a7cf3f1489f0a712c3e37e83af76e29bb5768a9a552fcd255e0c` |
| `gateway-aarch64-unknown-linux-gnu` | 19958960 | `17c1fd546d1a8fbb7d310bc3f62a872f23b9d1b5b3f8195e383fc9f86162bdf2` |
| `gateway-build-metadata.json` | 261 | `e603c554cb818ccde8a16bb1aa6451497d355152d090b13ffcc644ec97f738cb` |
| `gateway-image.oci.tar` | 41244160 | `e4503dca17842ba3869758ac5e914a13be43c23428ff0cc7981dde2d07385f94` |
| `gateway-sbom.cdx.json` | 209362 | `8dd60d22dec5b093e55c108534eb7f9e2b2920e6552fdcc3e579808dacbd885a` |
| `signing-identity.json` | 295 | `b3366b12c40998650c63999b5f041f1c2a787f965f8aa20413aa6c1b8bc10509` |

ELF64 little-endian/AArch64 (`e_machine=183`)、动态依赖全部解析、实际 `--help` 以及唯一嵌入 revision/Rust/target 均为 PASS；没有 development 身份。SBOM 使用既有 generator（CycloneDX 1.5，296 components）。OCI 使用原 Dockerfile、固定 Debian ARM64 base，实际载入后执行 `--help`，读取 `/usr/local/bin/gateway` 与交付 binary 逐字节 SHA 一致。

P12 严格 unsigned verifier 为 PASS；五个负例（缺 SBOM、binary digest mismatch、额外 source 文件、错误 revision、要求 signature/receipt）均按预期拒绝。`signing-identity.json` 只表示预期 GitHub OIDC 身份；Sigstore 签名、密码学验签均 **NOT_RUN**，没有 signature、bundle 或 verified receipt。未签名候选不是正式版本。

单一最终输入只有一次完整 release build；**TRACEABLE，reproducibility=NOT_RUN**。未执行两份独立相同输入副本/独立 target 的 reproducibility 实验，不因有 source archive 就宣称可重复。

## 4. 同一 binary 的 Linux reader/serve 资格

最终运行回执：[linux-qualification.json](assets/cpar66-local-candidate-20261022/linux-qualification.json)。scope 为 `stock_linux_arm64_reader_serve_only`；144/144 检查、5 个 serve 进程、22 个 reader 场景及 6 次覆盖拒绝（共 28 次 reader exec）。

| 检查 | 结果与证据边界 |
|---|---|
| 新状态 serve 与 readiness | PASS_BOUNDED；schema38；独立 data/admin listener，healthz/HEAD、data unauthorized401、admin 隐藏404 与正确 key 的读取；实际 `/proc/PID/exe` SHA 等于 manifest binary |
| metadata reader | PASS_BOUNDED；旧 v1 与 Claude v2 exact raw/source/owner、真实 Attempt；错误 owner/request/Attempt 为 exact unknown；错误 key 非零且无输出 |
| Usage / summary reader | PASS_BOUNDED；native Usage exact raw/identity，summary 完整有序内容/执行身份；同样覆盖授权与拒绝 |
| restart/adoption | PASS_BOUNDED；首进程退出后新的 PID/starttime 采用持久 marker 和 schema38 DB；encrypted rows 未改变，v1/v2 再次授权读回 |
| 私密边界 | PASS_BOUNDED；0700 目录/0600 证据、固定无值 stdout/stderr、拒绝覆盖、ordinary logs 无私密 sentinel；DB/key/plaintext 留在外部私有目录 |
| 实际隔离 | PASS_BOUNDED；gateway/reader UID65532、CapEff/CapBnd0、NoNewPrivs；外部 IPv4/IPv6 和活跃无关 loopback 被拒，允许本轮接收端。receiver 仅收到显式 GET probe，没有 inference/Provider 请求 |
| 退出与资源 | PASS；每个 serve exit0/PID不存在、data/admin listener 释放、receiver线程/listener释放；最终容器/VM实际状态另见 cleanup receipt |

这是本机 Colima 内的原生 Linux ARM64 资格，runtime 使用固定 builder 中的 Debian 12 userspace；OCI runtime image 本轮验证 load/help/embedded binary 对应，未另宣称完整 OCI 容器 serve 系统资格。也未机械重跑旧 267 项恢复矩阵、旧 Linux 系统操作或任何远端环境。

## 5. 验证、审查与继承保留

| 证据 | 结果 |
|---|---|
| Fresh overlay Full | **45/45 PASS**，Rust **1659 passed / 0 failed / 21 ignored**；受测源码 `1a8b6fc4`，与最终候选的唯一实际输入差异为 reader driver 固定流 oracle；Rust、fixture、lock、SPA、工具链输入相同 |
| 最终 driver 增量 | py_compile、5 个 stdout/stderr 边界用例、独立窄审查以及最终 binary 的完整 bounded Linux 运行；Full 不改名为新 SHA 的重跑 |
| RustSec | advisory/policy PASS；yanked query **UNKNOWN_TIMEOUT**，没有抹去该未知状态或无新线索重复查询 |
| Fixture export | 3 个 ignored exporter 显式执行 PASS，来源为 Full 受测候选、同一 fixture 实现；合成私密材料不入 Git |
| Source verifier | bytes/mode/link 负例全部准确拒绝；最终 source/extracted source/build 前后各 7,380 输入核对 PASS |
| Strict artifact / runtime | 见 manifest、OCI binding 与实际运行回执；不能以旧 workflow fixture 替代 |
| 两轴审查 | 初次各发现同一个 P2（去重 1）；已修复并经静态窄审查；source 权限绑定和最终 reader 修正均 0 新 finding。早期中断 scouts 与 partial review 仍如实保留 |
| 最终文档检查 | `scripts/check.sh docs` **7/7 PASS**，见 [docs-check.md](assets/cpar66-local-candidate-20261022/docs-check.md)；与受测源码/二进制分列 |
| 继承保留 | 18 项 bytes/mode/type/link/uid/gid 保持；15 项0600也保留；只提交本轮增量，未 reset/clean/stash/整体暂存 |

[verification-reuse.json](assets/cpar66-local-candidate-20261022/verification-reuse.json) 明确比较了产品输入：相对历史实际客户端候选 `4015bd4542dbc52fde8984c14c01b34843ef3c59`，产品 Rust、锁文件、工具链、SPA 与原构建入口无 diff。此前 Codex/Pi 与 Claude D2 controlled PASS 仅按该范围复用，未由本 Linux binary 重跑；原样 Claude 仍 BLOCKED_BEFORE_FIRST_RESPONSE。最终 build 的 embedded revision 不同也已明确记录。

## 6. 可审查材料与保留资格

外部 output 保留 source.tar、完整 input/overlay/lock 绑定、builder/image/proxy/API/版本与缓存证明、完整 build/Full/运行 logs、严格 verifier 与负例，以及私有 fixture/DB/key/reader 输出。仓库仅提交可公开审查的 manifest/回执、压缩输入清单与报告；私密目录、二进制、OCI/source archive 留在上述本地路径。

关键入口：

- [candidate-binding.json](assets/cpar66-local-candidate-20261022/candidate-binding.json)：source → environment/build → binary/SBOM/OCI/manifest → runtime 的原始文件 hash 绑定。
- [source-binding.json](assets/cpar66-local-candidate-20261022/source-binding.json)、[overlay.json](assets/cpar66-local-candidate-20261022/overlay.json)、[压缩输入清单](assets/cpar66-local-candidate-20261022/input-manifest.json.gz)。
- [artifact-manifest.json](assets/cpar66-local-candidate-20261022/artifact-manifest.json)、[OCI binding](assets/cpar66-local-candidate-20261022/oci-binary-binding.json)、[最终 Linux 回执](assets/cpar66-local-candidate-20261022/linux-qualification.json)、[cleanup](assets/cpar66-local-candidate-20261022/cleanup.json)。
- [Full](assets/cpar66-local-candidate-20261022/full-summary.json)、[reviews](assets/cpar66-local-candidate-20261022/reviews.json)、[failure history](assets/cpar66-local-candidate-20261022/development-history.json)、[preservation](assets/cpar66-local-candidate-20261022/preservation.json)。

复核入口按顺序：`cpar66-source.py verify` 核对 source archive 展开副本；用 `release-build.json` 的固定 image/独立 source+target/offline 命令执行 `cpar66-build.sh`；用既有 `p12-release-artifact.rb` 的 build-metadata/identity/manifest/verify 与归档的 exact assembly commands 组成六文件 set；最终用 `linux-run-command.json` 的独占 Linux envelope 运行该 set 中的 gateway。重新构建需要新的独占目录/container/volume 名称，不能直接覆盖本报告证据。

| 范围 | 最终状态 |
|---|---|
| 本地完整 unsigned ARM64 制品 | PASS_LOCAL_PREPARATION_BOUNDED |
| 同一 binary Linux reader/serve/restart/isolation/exit | PASS_BOUNDED |
| 三客户端受控闭环 | **REUSED** 历史同产品输入证据；本 binary **NOT_RUN** |
| 固定 Build 服务资格、#66整体 | **BLOCKED / OPEN**；#62/#64/#65 不解除 |
| D2 首版采用 | **NOT_DECIDED**；原样 Claude 控制承诺保留 |
| #61 `cost_in_usd_ticks` | **NOT_ADMITTED**；继续等待一手依据，未重复研究 |
| 两份独立构建可重复性 | **NOT_RUN** |
| 远端 CI、Sigstore、密码学验签、#67真实 Provider | **NOT_RUN** |
| 正式版本、发布、部署、旧远端 system epoch | **NOT_RUN** |

本轮新建专属 `cpar66-local-20261022`：原生 VZ ARM64、4 CPU、6 GiB RAM、40 GiB disk，activate/ssh-config 均禁用。释放前已导出精确 builder（767,972,352 bytes）与 dependency cache（86,343,680 bytes）。本轮容器、volume、BuildKit 均已移除，VM/data/profile 已实际停止并删除；task socket/profile 与 VM agent PID 均不存在，host 19180/19181/19443/19999 无监听。既有 default、cpar-acceptance-20261010、omp-pi130-01-20261018 均继续 Stopped，全局 Docker context 仍为 default。没有接管或清理旧环境。
