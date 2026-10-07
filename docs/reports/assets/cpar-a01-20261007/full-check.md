# A-01 新修复源码的 Full 执行回执

源码提交：`42bc473f2eba8c34484b54aaf09fff455aa13c9c`。本回执来自该提交的 clean detached checkout，非旧 source80。原始运行身份见 [full-receipt.json](full-receipt.json)。

# Check run log

- Mode: `full`
- Started: `2026-10-07T09:44:17Z`
- Host: `Darwin 27.0.0 arm64`

| Step | Duration seconds | Result |
|---|---:|---|
| Shell syntax | 0 | PASS |
| CI workflow | 0 | PASS |
| Release artifact workflow | 1 | PASS |
| CI change classifier | 0 | PASS |
| Plan state | 1 | PASS |
| Plan state guard | 0 | PASS |
| Canary thresholds | 0 | PASS |
| Caddy split | 1 | PASS |
| Caddy split guard | 1 | PASS |
| Management SPA dependencies | 1 | PASS |
| Benchmark baseline comparator | 1 | PASS |
| Soak runner argument guard | 29 | PASS |
| Soak receipt checker | 1 | PASS |
| Quality installer cache behavior | 2 | PASS |
| Release artifact verifier | 5 | PASS |
| P12 systemd unit | 0 | PASS |
| P12 OpenAI differential graph | 0 | PASS |
| P12 OpenAI differential classifier | 0 | PASS |
| P12 OpenClaw migration dry-run | 1 | PASS |
| P12 G1 production graph | 0 | PASS |
| P12 G1 live harness | 0 | PASS |
| P12 G1 Chat SSE classifier | 1 | PASS |
| P12 G1 Responses JSON classifier | 0 | PASS |
| P12 G1 Messages SSE classifier | 0 | PASS |
| P12 grok2api memory export | 0 | PASS |
| P12 Autoreg Console memory export | 0 | PASS |
| P12 production observer | 0 | PASS |
| P12 Grok CPAR curl E2E harness | 0 | PASS |
| CPAR new-build acceptance oracle | 4 | PASS |
| Management SPA | 2 | PASS |
| Rust format | 2 | PASS |
| Clippy | 17 | PASS |
| Rust tests | 147 | PASS |
| P12 serve envelope | 19 | PASS |
| Agent multi-turn and stream regression | 7 | PASS |
| Source policy | 1 | PASS |
| Secret scanner test | 1 | PASS |
| Crate boundaries | 0 | PASS |
| Document links | 0 | PASS |
| Contract tests | 1 | PASS |
| Tracked secret scan | 37 | PASS |
| Git whitespace | 1 | PASS |
| Quality tool versions | 0 | PASS |
| Dependency policy | 2 | PASS |
| RustSec audit | 6 | PASS |

Completed: `2026-10-07T09:49:09Z`
