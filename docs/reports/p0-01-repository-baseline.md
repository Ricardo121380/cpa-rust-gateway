# P0-01 repository baseline report

| 字段 | 值 |
|---|---|
| 计划 | `v1.0` |
| Task | `P0-01` |
| 日期 | `2026-07-18` |
| 分支 | `codex/p0-01-repo-baseline` |
| Commit | `b677113` |
| 结果 | PASS |

## 交付物

- 独立 Git 仓库与 `main` 分支。
- `.gitignore` 中的凭据、运行时数据库、日志和构建产物规则。
- 仓库内置 `scripts/secret-scan.sh`。
- `.githooks/pre-commit`，并通过 `core.hooksPath=.githooks` 启用。
- [安全策略](../../SECURITY.md)。

## 验证证据

| 验证 | 结果 |
|---|---|
| `bash -n scripts/secret-scan.sh` | PASS |
| `bash -n .githooks/pre-commit` | PASS |
| `scripts/secret-scan.sh --staged` | PASS |
| 暂存包含合成 API Key 的临时 canary 后尝试提交 | 按预期退出 `1`，仅报告文件名，不打印匹配值 |
| 删除 canary 后提交正式基线 | PASS |
| 提交后的 `git status --short --branch` | 工作树干净 |

Canary 未进入任何成功提交，且已经从工作树和索引移除。
