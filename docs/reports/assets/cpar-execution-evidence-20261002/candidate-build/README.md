# 本轮 native ARM64 配方与来源

固定源码 `80f5838902eb56be6dccc3ce047eb227d6c18e6c`；配方脚本在证据交付提交中保存，但不改变固定源码。执行目录 `/Users/huangrui/.codex/tmp/cpar-execution-20261002/`。完整独立 clone `source/` 的 tracked status 在构建前后为空，`.git` 自包含，未挂载含用户修改的原仓库。

构建 VM 是原生 aarch64 Colima Linux，临时启用4CPU/8GB；实际 build container 3CPU/6GB、Cargo jobs=2。Rust/Node pinned image 见 [Builder.Dockerfile](Builder.Dockerfile)，apt 工具实际版本在 [日志](../builder-toolchain.log)。宿主只复制已有 Cargo registry 到任务独立缓存，没有复制 Cargo auth/config。只通过本进程 `GIT_CONFIG_COUNT=1 / GIT_CONFIG_KEY_0=safe.directory / GIT_CONFIG_VALUE_0=/workspace` 允许任务挂载目录；没有修改全局 Git 设置。

1. `docker build --platform linux/arm64 -f Builder.Dockerfile -t cpar-execution-builder:20261002 .`，固定 builder image digest 见 [receipt](../candidate-receipt.json)。
2. 运行 [Native-build.sh](Native-build.sh)：mount 独立 source→`/workspace`、Cargo缓存→`/cargo`、候选目录→`/candidate`；设 `CARGO_HOME=/cargo`、`CARGO_BUILD_JOBS=2`、`GATEWAY_RELEASE_REVISION` 为固定SHA、`GATEWAY_RELEASE_RUST_VERSION=1.97.1`、`GATEWAY_RELEASE_TARGET=aarch64-unknown-linux-gnu`。调用项目 `scripts/build-release.sh`。Rust/cargo1.97.1，Node24.19.0。
3. 在同一固定源码 checkout 运行 `CARGO_NET_OFFLINE=true ./scripts/generate-p12-01-sbom.rb --target aarch64-unknown-linux-gnu --output <candidate>/gateway-sbom.cdx.json`。project pinned cargo-cyclonedx0.5.9；输出不含本地路径或credentials。
4. Buildx 只创建任务自己的 `cpar-execution-artifact-20261002` builder，未 `--use` 改变默认 builder。构建上下文只有 gateway ELF；使用固定源码 Dockerfile，`--platform linux/arm64` 和原 P12 pinned Debian base。release revision/version/Rust/target build args与metadata相同；`--provenance=false` 保持 P12 单 manifest OCI 结构，导出 OCI tar 和任务私有 smoke Docker archive。
5. 执行原 P12 `manifest` / unsigned `verify`，全部五个 payload 与 manifest 校验。OCI image nonroot、network none、read-only 执行 `--help`。本地不伪造签名或workflow receipt；`--require-signature` 拒绝缺少签名的目录。
6. [Native-test.sh](Native-test.sh)在 network none builder container 中运行现有真实gateway+隔离SQLite+本地TLS/SOCKS harness，用 `CPAR_GATEWAY_BINARY=/candidate/gateway-aarch64-unknown-linux-gnu` 与 `TMPDIR=/private`。所有 provider 都为任务自有loopback，真实调用0；测试收集410/410和binary/source hash。
7. [Artifact-compatibility.py](Artifact-compatibility.py)对上述私有合成状态副本降为31，再用实际候选启动迁移到34，读取受保护actual build信息、逐行比较14表原列和legacy未知字段；恢复完整副本为31并逐项核对。它只操作 `/private/cpar-schema-*`，raw数据留私有目录。[Execution-examples.py](Execution-examples.py)只读已关闭并checkpoint的fixture导出8个有界事实样本，绝不用于live数据库忽略WAL。
8. [Production-readonly.py](Production-readonly.py)只读systemd/实际proc ELF、schema/integrity、路径/容量；没有启停服务、provider调用或credential内容输出。[正式入口](../../../../../scripts/acceptance/cpar-new-build-acceptance.md)的 inventory GET 在目标主机内部使用既有管理认证，仅导出allowlist。

失败与修正：首次c94b35ed native编译成功，包装脚本误用未支持的`--version`；后续Git挂载属主检查和`ldd|head`在pipefail下的141也如实保留。最终80f58389使用进程级safe.directory、完整`ldd --version`及`--help`，[最终构建](../native-build.log) exit0。前三个失败日志不作为最终候选成功证据。

候选整体复制到原仓库独立 `output/cpar-execution-evidence-20261002/linux-arm64/<sourceSHA>/`，其他output文件保留。没有Docker prune或删除无关卷/容器；任务builder收尾移除并恢复原本停止的Colima。保留任务自己的候选/缓存，不作为全局迁移或安装。

日志导出仅规范CR/LF、行尾空格和终端EOF空行，以通过仓库whitespace规则。改变过的原始字节保存在任务output/logs-raw；原hash记录在evidence-manifest和Full回执，结果/命令/日志文本未筛除。
