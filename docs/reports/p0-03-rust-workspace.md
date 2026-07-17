# P0-03 Rust Workspace report

| 字段 | 值 |
|---|---|
| 计划 | `v1.0` |
| Task | `P0-03` |
| 日期 | `2026-07-18` |
| 分支 | `codex/p0-03-rust-workspace` |
| Rust | `rustc 1.97.1 (8bab26f4f 2026-07-14)` |
| Cargo | `cargo 1.97.1 (c980f4866 2026-06-30)` |
| 结果 | PASS |

## 交付物

- 固定 `rust-toolchain.toml`：Rust `1.97.1`、`rustfmt`、`clippy`。
- Edition 2024 根 Workspace 与 `Cargo.lock`。
- 1 个应用和 20 个库，共 21 个 Workspace package。
- Actix Web 只由 `gateway-http-actix` 直接依赖；核心领域 Crate 不依赖 Actix 类型。
- [Crate 依赖边界](../crate-boundaries.md)及可执行检查器 `scripts/check-crate-boundaries.rb`。
- Workspace 默认 `unsafe_code = deny`，并继承基础 Rust/Clippy lint。

## 验证证据

| 命令 | 结果 |
|---|---|
| `rustc --version` | `1.97.1` |
| `cargo --version` | `1.97.1` |
| `cargo metadata --format-version 1 --no-deps` | PASS；21 个 Workspace package |
| `scripts/check-crate-boundaries.rb` | PASS；21 个 package 的精确允许边一致 |
| `cargo fmt --all -- --check` | PASS |
| `cargo check --workspace` | PASS；clean rebuild 22.97s，warm check 0.25s |
| `scripts/check-doc-links.rb` | PASS；15 份 Markdown |
| `git diff --check` | PASS |

## 边界说明

P0-03 只建立可编译模块边界和进程占位入口，没有实现 HTTP 路由、Canonical 类型、Provider 行为或持久化逻辑。对应业务能力从 P1 开始按计划逐 Task 实现。
