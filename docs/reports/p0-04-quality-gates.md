# P0-04 quality gates report

| 字段 | 值 |
|---|---|
| 计划 | `v1.0` |
| Task | `P0-04` |
| 日期 | `2026-07-18` |
| 分支 | `codex/p0-04-quality-gates` |
| cargo-deny | `0.20.2` |
| cargo-audit | `0.22.2` |
| 结果 | PASS |

## 交付物

- `rustfmt.toml`、`clippy.toml` 和 Workspace lint priority。
- `deny.toml`：许可证 allowlist、Registry/Git 来源、Wildcard、Yanked 和 Advisory 策略。
- [质量门禁说明](../quality-gates.md)。
- `scripts/check-source-policy.rb`。
- `scripts/test-secret-scan.sh`。
- `apps/gateway/tests/component_smoke.rs`，验证进程入口只链接预期顶层边界。

## 最终验证证据

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --workspace --all-features` | PASS；1 个集成冒烟测试通过 |
| `scripts/check-source-policy.rb` | PASS；22 个 Rust 文件、21 个 Crate 根 |
| `scripts/test-secret-scan.sh` | PASS；安全文件允许、合成 Secret 拒绝且不泄露值 |
| `scripts/check-crate-boundaries.rb` | PASS；21 个 package |
| `scripts/check-doc-links.rb` | PASS；17 份 Markdown |
| `cargo deny check` | PASS；advisories/bans/licenses/sources 全部通过 |
| `cargo audit` | PASS；加载 1166 条 RustSec Advisory，扫描 137 个依赖 |
| `scripts/secret-scan.sh --all` | PASS |
| `git diff --check` | PASS |

## 已评估告警

`cargo-deny` 报告 `socket2 0.5.10` 与 `0.6.5` 两个版本。前者来自 `actix-server 2.6.0`，后者来自 `actix-web 4.14.0`/`tokio 1.53.0`。两者均由同一 Actix 依赖闭包引入、没有 RustSec 告警，P0 不使用强制 patch；`multiple-versions = "warn"` 保留以便后续依赖升级时自动复核。

## 本机工具链说明

首次编译 `cargo-audit` 时继承了 Miniconda 的 `CC/CFLAGS/LDFLAGS`，导致 `aws-lc-sys` 链接失败。清空这些 flags 并显式使用 `/usr/bin/clang` 后成功安装。P0-05 的开发工具安装入口必须主动隔离这些环境变量，避免本地与 CI 行为漂移。

P0 未添加任何 Advisory、License 或 Source 例外。
