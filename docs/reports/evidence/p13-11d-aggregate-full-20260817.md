# P13-11D aggregate local Full receipt — 2026-08-17

Status: `LOCAL_PASS_PENDING_PHASE_GATE`

## Scope

This receipt covers the local aggregate Full for P13-11D after D1 typed persistence/AEAD, D2
protected management contract, and D3 active runtime composition. It is not a formal GitHub
Delivery Gate and does not authorize Provider, proxy, DNS, server, staging, or production traffic.

## Candidate lineage

- Branch: `codex/p13-11-egress`
- D3 implementation commit: `5bd04a7`
- Crate-boundary allowlist correction: `acf4e47`
- Prior D3 documentation commit: `d9474e7` (this durable receipt is committed in the follow-up
  evidence commit)
- Host: `Darwin 25.2.0 arm64`
- Pre-existing untracked helper files: preserved outside the candidate and not staged.

## Full command and result

```text
CHECK_REPORT_PATH=/tmp/cpar-p13-11d-aggregate-full-final.md ./scripts/check.sh full
```

- Started: `2026-08-17T01:41:24Z`
- Completed: `2026-08-17T01:43:02Z`
- Result: `PASS`
- Completed steps: `43/43`

All workflow and plan guards, management SPA/contract checks, Rust format, Clippy, Rust tests,
P12 serve envelope, source policy, secret scanner, crate boundaries, document links, contract
references, tracked secret scan, whitespace, quality/dependency policy, and RustSec audit passed.

The first attempt stopped at crate-boundary validation because `gateway-control/Cargo.toml` already
declared the required `sha2` dependency while `scripts/check-crate-boundaries.rb` omitted it from
the allowed set. No test or security failure occurred. The allowlist was corrected in `acf4e47`
and the same Full command was rerun successfully.

## Boundary

This receipt proves local source/build/review invariants only. It does not prove that any proxy,
DNS route, Provider endpoint, account, Grok Web/Console session, Autoreg flow, server, staging
deployment, or production traffic works. Formal P13-11D closeout remains pending explicit phase
authorization.
