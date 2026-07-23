# G10 management-control-plane gate report

| Field | Value |
|---|---|
| Plan | `v1.44` |
| Gate | `G10` |
| Date | `2026-07-24` |
| Verification branch | `codex/p10-control-plane` |
| Local result | `PASS` — P10-01 through P10-09 and the integrated local Full Gate meet the local G10 acceptance conditions. |
| Delivery state | `PASS` — the annotated [`phase-p10-complete`](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/30047169917) Delivery Gate completed with Classify, Fast, Full supply-chain, and Required successful. |

## Gate-condition evidence

| G10 condition | Evidence | Local result |
|---|---|---|
| UI/API can configure a two-station `minimax-m3` aggregate | `p10_05_management_routing` creates two separately owned Upstream/Endpoint/Credential/Binding chains, attaches both as enabled Candidates to one `minimax-m3` Route, and validates the complete Route through protected management HTTP. P10-04/P10-05 browser flows independently prove that the generated SPA owns the same protected resource operations without browser persistence. | PASS |
| Secrets are display-once or masked and cannot be read back as plaintext | P10-01's frozen schema allows credential secret only as write input and Client Key only in the explicit issuance response. P10-04 seals Credential input before persistence; P10-05 verifies both station credential responses and every later Client Key read omit the values. P10 SPA checks reject browser storage, cookies, clipboard and direct fetch. | PASS |
| Failed publication retains the prior data-plane Snapshot | P10-07 delegates to the P2 publication boundary; the integrated workspace suite includes `compile_failure_leaves_the_active_snapshot_and_draft_version_unchanged`, and lifecycle HTTP tests cover draft-only publication, stale rejection and retained-predecessor rollback. | PASS |
| An empty database restores from a backup and passes `SQLite quick_check` | P10-08's encrypted store integration creates a bounded XChaCha20-Poly1305 snapshot, preflights it, restores only into an absent target, preserves versioned configuration, and rejects wrong keys, tampering, malformed material and existing targets. Its protected HTTP tests enforce binary bounds and never accept a caller-selected path. | PASS |
| Enabling the management UI has no material data-plane effect | P10-09 makes UI registration explicit and absent from public `configure`; isolation tests prove public data-plane configuration has no UI route and UI-only configuration has no health route. The route-level 2,000-request structural comparison completed with each configuration, while quantitative P99/RSS/throughput thresholds remain P11 scope. | PASS |

## Integrated verification and review

| Check | Result |
|---|---|
| `cargo fmt --all -- --check`, workspace Clippy, workspace tests | PASS — including all P10 protected HTTP, backup, UI-isolation and two-station aggregate regressions. |
| `npm --prefix web/admin-ui run check` | PASS — 65 frozen OpenAPI operations and reproducible static build. |
| Source/crate-boundary policy, docs links, tracked/all-file Secret scans, whitespace | PASS. |
| `cargo deny check`, `cargo audit` | PASS; expected duplicate crate-version warnings are policy-visible and non-fatal. |
| Phase review | PASS after extending P10-05 from one station to a verified two-station aggregate. Review also reconfirmed management/data-plane route separation, authenticated encrypted artifacts, empty-target-only restore, generated-client-only SPA traffic, default-deny management facades and no production/deployment change. |

## Delivery boundary

No Provider request, deployment, management listener bind, server configuration change, production
Feature Flag, or external account state was introduced for G10. The P10 closeout target's one
GitHub Fast + Full Delivery Gate passed, so P10 is `DONE` and P11 may begin on a fresh P-level
branch. This P11-first-task reconciliation does not create another P10 tag or CI event.
