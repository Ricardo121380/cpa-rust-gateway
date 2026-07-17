# P0-05 local and CI baseline report

| 字段 | 值 |
|---|---|
| 计划 | `v1.0` |
| Task | `P0-05` |
| 日期 | `2026-07-18` |
| 分支 | `codex/p0-05-ci-baseline` |
| 被测实现 Commit | `b2ea249` |
| 结果 | PASS |

## 交付物

- `scripts/check.sh fast`：格式、Clippy、测试、源码、Secret、架构、文档、Workflow 和 whitespace。
- `scripts/check.sh full`：在 fast 基础上增加固定版本工具检查、`cargo-deny` 与 `cargo-audit`。
- `scripts/install-quality-tools.sh`：固定且校验 `cargo-deny 0.20.2`、`cargo-audit 0.22.2`，隔离本机 Conda 编译 flags。
- `scripts/verify-clean-checkout.sh`：独立 clone、detached commit、全新 `CARGO_TARGET_DIR` 验证。
- `.github/workflows/ci.yml`：Ubuntu 24.04 fast/full 两个 Job，full 依赖 fast。
- `actions/checkout` 固定到完整 commit SHA，不使用浮动 Action Tag。
- `scripts/check-ci-workflow.rb`：YAML 语法、必需 Job/命令和 Action SHA 校验。

## 本地与 CI 一致性

GitHub Workflow 不重新实现门禁，只调用仓库中的相同入口：

```text
Fast Job -> ./scripts/check.sh fast
Full Job -> ./scripts/install-quality-tools.sh -> ./scripts/check.sh full
```

因此本地和 CI 的命令、工具版本、参数与失败语义来自同一受版本控制脚本。

## 干净检出验证

对 commit `b2ea249` 创建独立临时 clone，使用独立空 `CARGO_TARGET_DIR` 执行：

```text
./scripts/verify-clean-checkout.sh HEAD full docs/reports/p0-05-clean-checkout-log.md
```

[逐步骤日志](p0-05-clean-checkout-log.md)记录 14 个步骤全部 PASS；Clippy 与测试重新编译依赖，日志中的源路径位于临时 clone。验证结束时临时源码工作树保持干净。

## 托管状态

当前本地仓库没有 Git remote，因此本 Task 没有伪造“已运行 GitHub Hosted Runner”的结论。Workflow 的 YAML、受控入口和干净 clone 行为已验证；配置 remote 后首次 push 必须取得 Hosted fast/full 两个绿色 Job，才可把 GitHub 平台状态视为已验证。
