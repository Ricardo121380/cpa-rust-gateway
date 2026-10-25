# Tokio 1.53.3 安全维护

日期：2026-10-24（Asia/Shanghai）。用户授权修复上一轮完整门禁中的
RUSTSEC-2026-0386 阻碍；历史 [effort 实施记录](cpa-v8-messages-effort-20261024.md)
保留当时的 FAIL 和受测源码身份。

## 修改与依据

当前 checkout：`worktrees/cpar62-client-metadata-20261020`，分支 `feature/62-client-metadata`，
HEAD `ab78c65ca68978bf46c7cf530ea795417d8431f3` 加全部继承修改和本轮依赖增量。
HEAD 本身不是受测源码。

- [Cargo.toml](../../Cargo.toml) 的 Tokio 最低版本由1.53.0改为1.53.3，features不变。
- 执行 `cargo update -p tokio --precise 1.53.3`；[Cargo.lock](../../Cargo.lock)仅 Tokio
  package 的版本和 checksum 改变。新 checksum 为
  `ce3335fa71841cda333a58d7615b03901380ecf09d59b3296d21f8bbac0dde4e`。
- 保留 deny/audit 规则，未添加 advisory ignore；本轮没有改动 Rust 产品代码或测试预期。

[固定 RustSec 公告](https://raw.githubusercontent.com/RustSec/advisory-db/84d943a6deeb8cc6c6f632531aaf4f1c62275239/crates/tokio/RUSTSEC-2026-0386.md)
将 >=1.53.3 列入修复范围。
[Tokio 1.53.3 变更记录](https://raw.githubusercontent.com/tokio-rs/tokio/tokio-1.53.3/tokio/CHANGELOG.md)
包含 AsyncFd API 修复及运行时、同步等补丁；
[其 Cargo manifest](https://raw.githubusercontent.com/tokio-rs/tokio/tokio-1.53.3/tokio/Cargo.toml)
要求 Rust1.71，低于项目固定的1.97.1。选择当前版本线的最小修复补丁。

限定检索 apps/crates/tests/tools/.cargo 未定位直接 AsyncFd/raw-fd 调用或 tokio_unstable/
io_uring flags；这不构成传递依赖无风险的判断。依赖包数和依赖边不变，其他 Tokio 使用方
继续继承 workspace。原有14项重复版本告警逐项与上一轮 deny 输出一致，涉及 block-buffer、
chacha20、const-oid、cpufeatures、crypto-common、digest、http、rand_core、shlex、socket2、
syn、thiserror、thiserror-impl、getrandom；本轮不引入新的重复版本，不扩展为传递依赖重构。

## 验证

| 检查 | 状态 |
|---|---|
| 修改前 cargo deny check advisories | RED，退出1，复现 RUSTSEC-2026-0386 |
| 修改后 cargo deny check | PASS，advisories/bans/licenses/sources均通过 |
| 修改后 cargo audit --json | PASS，退出0，vulnerabilities=0、warnings为空 |
| scripts/check.sh full | PASS，44/44，退出0；包含格式、Clippy、全部Rust测试、进程/多轮回归及供应链门禁 |
| Workspace Rust tests | PASS，132个测试集，1671 passed、0 failed、21 ignored；ignored不计为通过 |

本地回执目录：工作区根下 `maintenance/2026-10-24-tokio-security-fix/`。
`baseline.json`、`before/`、`index-before.bin` 保存原始内容；`dependency-delta.json`
确认仅 Tokio 版本/checksum 和 manifest 最低版本变化；`candidate-manifest.json`绑定
完整受测 overlay，SHA-256为
`9552acd89e1fe3e40181d7e935aafa01a9cc6346cd32591f9da4c9e627e56229`。`red-advisories.log`、`green-deny.log`、`audit.json`、`full-checks.md`
及 `full-checks.log` 保存独立结果。最终文档补记与受测代码/lock身份分开。

完整回归使用项目固定工具链，覆盖已存在的本地 HTTP/WebSocket、网络超时、队列背压、
取消/终态、持久化和协议兼容用例。实际客户端、真实 Provider、目标发布制品、远端 CI、
部署及发布均 NOT_RUN；本轮不改变首版或工单资格。

最终 preservation 校验：只修改3个已有文件，新增本维护记录；其他7492个原有路径及381项
既有删除状态保持，Git HEAD/index不变。Full结束后仅补记本报告与统一入口，受测依赖和
其余源码身份保持。此次没有提交、推送或远端写入。
