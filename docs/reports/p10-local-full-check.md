# Check run log

- Mode: `full`
- Started: `2026-07-23T21:30:30Z`
- Host: `Darwin 25.2.0 arm64`

| Step | Duration seconds | Result |
|---|---:|---|
| Shell syntax | 0 | PASS |
| CI workflow | 0 | PASS |
| CI change classifier | 0 | PASS |
| Plan state | 0 | PASS |
| Plan state guard | 1 | PASS |
| Quality installer cache behavior | 0 | PASS |
| Management SPA dependencies | 0 | PASS |
| Management SPA | 2 | PASS |
| Rust format | 0 | PASS |
| Clippy | 16 | PASS |
| Rust tests | 140 | PASS |
| Source policy | 0 | PASS |
| Secret scanner test | 0 | PASS |
| Crate boundaries | 1 | PASS |
| Document links | 0 | PASS |
| Tracked secret scan | 6 | PASS |
| Git whitespace | 0 | PASS |
| Quality tool versions | 0 | PASS |
| Dependency policy | 2 | PASS |
| RustSec audit | 2 | PASS |

Completed: `2026-07-23T21:33:20Z`

## P10 closeout re-run

After the G10 phase review extended `p10_05_management_routing` from a one-station graph to a
two-station `minimax-m3` aggregate, the same command was run again on `2026-07-24` and passed.
This re-run included the expanded protected routing test, P10-08 encrypted restore checks and
P10-09 embedded-UI isolation checks, alongside the existing workspace, SPA, policy, documentation,
Secret, dependency and RustSec checks. The complete terminal result was `check: full passed`.
