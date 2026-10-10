# Initial independent review

Fixed scope: `82a1f69861300456cca7df779ee07949d043c84b...98febf057f37bd784e81e9a2563adb36aef8ebbf`.
Agents were independent, one-shot and read-only. They ran no tests or remote operations.

## Standards

Agent `/root/build_live_standards_review`: **PASS**. Hard violations 0; actionable judgement smells 0; highest severity none.

Covered all six changed files: `apps/gateway/src/deployment.rs`, `apps/gateway/src/main.rs`, `scripts/acceptance/README.md`, `scripts/acceptance/cpar_seo_ui_http.py`, `scripts/acceptance/cpar-background-provider-process.py`, `scripts/acceptance/cpar-build-egress-guard.c`.

The closed CLI mode, worker subscription gate, required worker shutdown, existing startup refresh seam, process acceptance and atomic snapshot helper follow the repository ownership and lean change rules. No frontend-owned path changed. The documented listener/worker assembly lint exception is limited to that function. The two process controllers' API helpers each own their configuration/CAS state; the existing fixture has top-level launch effects, so their similar shape was not identified as actionable duplication.

Repository rule: `AGENTS.md:30` requires an entry when changing a path the other side owns. No such path appears in this diff. No separate CODING_STANDARDS.md or CONTRIBUTING.md was found in the bounded repository scan excluding output, target and .git.

Existing evidence was read, not rerun by the agent: Full source fb96f23c (45/45, Rust 1501/0/12) and fixed fixture source 98febf05 with unchanged product sources.

## Spec

Agent `/root/build_live_spec_review`: **PARTIAL**. One declaration boundary gap, P3. No established OAuth leak was found.

Covered the same six diff paths and related startup/publication code in runtime.rs, runtime/reload.rs and credential_refresh.rs.

Finding: `README.md:155` says "TCP/443 connections", but `cpar-build-egress-guard.c:97` checked address, resolved IP, port and audit, without checking the fd socket type/protocol. Static inference: UDP/443 connect to a resolved Build IP would also be admitted. The agent did not dynamically test this and did not claim stock traffic had used UDP or leaked OAuth.

The disabled CLI gate skips startup refresh and the entire start_workers subscription, so later publications do not recreate workers. Restart retains the original disabled command. The independent receiver receipts prove default refresh 2/catalog 2/inference 1 and disabled refresh 0/catalog 0/inference 1 after publication, restart, republication and a full 65-second interval. Three OAuth seed events belong to explicit synthetic setup, before the frozen worker baseline.

The guard fails closed within its documented getaddrinfo/connect and audit boundaries. Actual candidate loading, private copy, execution binding and real acceptance were still NOT_RUN at review. Global project specification and release/production acceptance were outside this scope.

Per-axis findings: Standards 0 (none); Spec 1 (P3 TCP-only declaration). Original verdicts remain historical after repair.
