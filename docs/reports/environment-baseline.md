# P0-06 environment baseline

| 字段 | 值 |
|---|---|
| 计划 | `v1.0` |
| Task | `P0-06` |
| 采集时间 | `2026-07-18 02:29-02:32 CST` |
| 本地环境 | MacBook Air |
| 目标环境 | Jakarta VPS |
| 结果 | PASS |

本报告只记录 P0 的可重复构建与后续基准环境，不把当前占位二进制的数据解释为网关运行时性能。

## 本地 Mac

| 项目 | 实测值 |
|---|---|
| 型号 | MacBook Air `Mac14,2` |
| SoC | Apple M2，8 核（4 Performance + 4 Efficiency） |
| 架构 | `arm64` |
| 内存 | 8 GiB（`8589934592` bytes） |
| OS | macOS `26.3.1`，Build `25D2128` |
| Kernel | Darwin `25.3.0` |
| 文件系统 | APFS，SSD |
| 根卷 | 228 GiB 视图；采集时约 12 GiB immediate available |
| Swap | 5 GiB total，采集时约 4.50 GiB used |
| Load | `3.02 / 4.16 / 4.35`；非空闲测量 |
| `ulimit -n` | `1048575` |
| Kernel max files/process | `10240`（未在 P0 调整） |
| Container CLI | Docker/Podman 均未安装 |

### 本地工具链

| 工具 | 版本 |
|---|---|
| rustc | `1.97.1 (8bab26f4f 2026-07-14)` |
| cargo | `1.97.1 (c980f4866 2026-06-30)` |
| clippy | `0.1.97` |
| rustfmt | `1.9.0-stable` |
| cargo-deny | `0.20.2` |
| cargo-audit | `0.22.2` |
| Apple clang | `17.0.0` |
| Git | `2.50.1 (Apple Git-155)` |
| Ruby | `2.6.10` |

Workspace 共 21 个 package。本地 Miniconda 会向原生编译注入 `CC/CFLAGS/LDFLAGS`；开发工具安装脚本已显式隔离这些变量。

## Jakarta VPS

采集通过已有 SSH Key 执行只读命令；没有写文件、安装软件、修改配置、重启服务或操作容器。

| 项目 | 实测值 |
|---|---|
| OS | Ubuntu `24.04` |
| Kernel | Linux `6.8.0-55-generic` |
| 架构 | `x86_64` |
| 虚拟化 | KVM full virtualization |
| CPU | 4 vCPU，Intel Xeon Platinum 8336C @ 2.30GHz |
| 拓扑 | 2 sockets × 2 cores，1 thread/core |
| 内存 | `8335015936` bytes total；采集时约 6.33 GB available |
| Swap | `2147479552` bytes total，采集时 0 used |
| 根卷 | ext4，约 41.96 GB total / 14.71 GB available / 64% used |
| 虚拟块设备 | `vda` 40G，ROTA flag `1` |
| `ulimit -n` | `65535` |
| `net.core.somaxconn` | `8192` |
| `vm.overcommit_memory` | `0` |
| Load | `0.48 / 0.13 / 0.04` |
| systemd | `255` |
| Docker client | `29.1.3`；deploy 用户无 daemon info 权限 |
| Git | `2.43.0` |
| rustc | `1.96.0` |
| cargo | `1.96.0` |
| rustup | `stable-x86_64-unknown-linux-gnu` |

## 构建基线

### 干净检出 full gate

[P0-05 clean-checkout 日志](p0-05-clean-checkout-log.md)使用独立 clone 和空 `CARGO_TARGET_DIR`：

| 项目 | 结果 |
|---|---|
| 全部步骤 | 14/14 PASS |
| Clippy | 12 s |
| Rust tests | 17 s |
| 总窗口 | 约 38 s |

### 本地冷 release build

命令使用全新的临时 `CARGO_TARGET_DIR`：

```text
cargo build --release --locked
```

| 指标 | 实测值 |
|---|---:|
| Cargo reported build time | 16.95 s |
| Wall time | 17.04 s |
| User CPU | 51.37 s |
| System CPU | 6.02 s |
| Maximum RSS | 381,108,224 bytes |
| 占位二进制大小 | 338,704 bytes |
| 二进制格式 | Mach-O 64-bit arm64 |

测试时本机并非空闲且 Swap 使用较高，因此只作为 P0 工程基线；P11 的正式性能基准必须在受控负载下重新建立。

## 对后续开发的约束

- 本地是 arm64/macOS，生产是 x86_64/Linux；Linux CI 制品才是服务器发布依据。
- 服务器 Rust `1.96.0` 低于项目固定的 `1.97.1`，不得在生产机临时使用现有工具链构建 Release。
- P12 应部署 CI 生成的固定二进制或镜像，并验证 checksum/SBOM，而不是现场编译。
- 本机只剩约 12 GiB immediate available；应复用 Cargo 缓存并定期监控 `target`，大型 Fuzz/Soak 使用独立存储或服务器 Staging。
- 服务器 4 vCPU/约 8 GB RAM 是后续并发、连接池、SQLite 队列和内存预算的真实上限。
- P11/P12 的性能报告必须记录相同字段，不能拿本机 arm64 数据替代服务器结论。
