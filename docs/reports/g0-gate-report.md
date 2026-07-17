# G0 phase gate report

| 字段 | 值 |
|---|---|
| 计划 | `v1.0` |
| Gate | `G0` |
| 日期 | `2026-07-18` |
| 验证分支 | `codex/g0-phase-gate` |
| 被测实现 Commit | `6bbf75c` |
| 结果 | `PASS` |

## 结论

P0-01 至 P0-06 均为 `DONE`，G0 的五项退出条件全部满足。P0 状态已切换为 `DONE`，当前阶段进入 P1，但 `P1-01` 保持 `PENDING`；本 Gate 没有实现 P1 业务功能，也没有修改服务器。

## G0 条件与证据

| 条件 | 证据 | 结果 |
|---|---|---|
| Workspace 在全新目录可重复构建 | [干净检出 full gate](g0-clean-full-log.md) | PASS |
| 所有 Crate 依赖方向符合目标架构 | `scripts/check-crate-boundaries.rb`，21 个 package | PASS |
| Workspace 默认禁止 unsafe 且没有已接受的例外 ADR | 根 `Cargo.toml` + `scripts/check-p0-gate.rb` | PASS |
| License Allowlist 不包含 AGPL/GPL/SSPL | `cargo deny check` + Gate auditor | PASS |
| CI 与本地使用同一受版本控制入口 | `scripts/check.sh`、`.github/workflows/ci.yml`、clean clone | PASS |

任务级证据见 [需求追踪索引](../traceability.md) 中 P0-01 至 P0-06 的报告链接。

## 执行记录

```text
REPRO_REPORT_PATH=docs/reports/g0-reproducible-build-log.md \
  ./scripts/verify-reproducible-build.sh HEAD

HTTPS_PROXY=http://127.0.0.1:7897 \
HTTP_PROXY=http://127.0.0.1:7897 \
ALL_PROXY=http://127.0.0.1:7897 \
  ./scripts/verify-clean-checkout.sh HEAD full \
  docs/reports/g0-clean-full-log.md

./scripts/check-p0-gate.rb
./scripts/check.sh full
```

- 干净检出完整门禁：14/14 步骤通过。
- 两个独立 clone 与 target 的 release 二进制均为 `354816` bytes。
- 两份二进制 SHA-256 均为 `ff17545dcee9d7a551bea329ede0a2ef7295a8fa3426b444a317c76c1e7e0708`。
- 两份运行输出均为 `gateway skeleton: 3 components linked`。
- 详细值见 [可复现构建日志](g0-reproducible-build-log.md)。

## 偏差、失败与处理

- 预验证发现对 Apple target 全局使用 `-Wl,-no_uuid` 会导致当前 macOS `dyld` 拒绝运行 build script；该方案未提交。最终实现保留合法 `LC_UUID`，由 release 构建脚本生成内容确定的 UUID 并使用固定 identifier 重新 ad-hoc sign。
- clean gate 的前两次尝试分别因 GitHub 直连超时和 `cargo-audit` 不识别 SOCKS proxy 失败。最终使用 Clash mixed port 的 HTTP proxy 原样重跑 full gate；没有跳过、离线替代或放宽任何检查。
- `cargo-deny` 报告 Actix 依赖树同时包含 `socket2 0.5.10` 与 `0.6.5`。这是非阻断 duplicate warning；advisories、bans、licenses、sources 与 RustSec audit 均通过。

## 已知限制与后续约束

- 本地仓库尚无 Git remote，因此未声称 GitHub Hosted Runner 已运行；Workflow 语法、固定 Action SHA、统一命令和干净 clone CI 等价路径已验证。首次配置 remote 并 push 后仍需取得 hosted fast/full 两个绿色 Job。
- macOS 的确定性规范化只作用于 release artifact；Linux/非 Mach-O 产物保持 Cargo 原始输出。P12 仍必须以 Linux CI 产物为服务器发布依据。
- 阶段完成标记为 annotated tag `phase-p0-complete`；回滚到该 tag 可恢复 P0 完成基线。
