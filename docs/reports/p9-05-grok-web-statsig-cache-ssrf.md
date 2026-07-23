# P9-05 Grok Web Statsig cache and SSRF report

| Field | Value |
|---|---|
| Plan version | `v1.42` |
| Task | `P9-05` |
| Date | `2026-07-23` |
| Branch | `codex/p8-official` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P9-LOCAL-001` |
| Scope / budget | `M`; in-memory exact signature cache and injected P2 admission seam. No Statsig/Web endpoint, browser/profile, SSO/Cookie source, server, proxy/TUN configuration, or HTTP request was used. |
| References | Matrix `C29`、`D28`、`E27-E29`; [ADR-0065](../adr/ADR-0065-grok-web-statsig-cache-ssrf-boundary.md); [BC-SEC-003](../contracts/BC-SEC-003-grok-web-statsig-cache-ssrf-boundary.md) |

## Delivered behavior

`provider-grok` now caches zeroizing Statsig signatures under only one exact `(method, path, environment version)` key. It uses caller time, finite capacity, expiry reclamation, and no live-key eviction. The 403 remediation primitive removes just the specified key, preserving every unrelated entry.

The signer boundary accepts injected P2 `EgressPolicy` and DNS resolver components. It requires HTTPS, delegates initial and redirect target admission to P2 exact allowlist/DNS/CIDR/redirect rules, redacts targets, and exposes no URL or send API. Tests use a static in-memory DNS resolver only.

## Verification and review

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p9_05_web_statsig` | PASS; three synthetic cache, 403, expiry/capacity, domain, redirect, HTTPS, and redaction tests passed. |
| `cargo fmt --all -- --check`, `cargo clippy --locked -p provider-grok --test p9_05_web_statsig -- -D warnings` | PASS. |
| `./scripts/check.sh full` | PASS; local workspace Full gate, plan-state guard, Rust format/Clippy/tests, supply-chain checks, docs checks, and Secret scan passed. |
| Focused review | PASS: no cache key can cross method/path/environment, 403 invalidation is exact, non-HTTPS stops before P2 resolution, redirects remain fully P2 re-admitted, and no HTTP send capability exists. |

## Deferred external proof

This local work does not prove a current Statsig signer URL, signature format, redirect behavior, or Web account response. P9-09/G9 remain deferred to a P9-specific test account and explicit Canary authorization. P8 Official E2E and P7 Kiro OAuth remain in the final external-authentication package.
