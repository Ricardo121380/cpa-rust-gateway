# P11-05 Security audit execution plan

| Field | Value |
|---|---|
| Plan version | `v1.45` |
| Task | `P11-05` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Branch | `codex/p11-release-hardening` |
| Task Card | A source-first, local security audit of SSRF/egress, Secret handling, public and management authentication, access control, dependency provenance, and a reproducible software bill of materials. |
| References | [P11 plan](../06-development-plan.md#17-p11---发布加固), [EgressPolicy report](p2-09-egress-policy-ssrf-admission.md), [Secret-store report](p2-03-aead-secret-store.md), [security policy](../../SECURITY.md), [SSRF contract](../contracts/BC-SEC-002-egress-policy-ssrf-admission.md) |

## Required acceptance

The audit must establish, with source locations and deterministic checks, that:

1. outbound URLs are policy-admitted, DNS answers are pinned per dial, dangerous address ranges and
   redirect escapes fail closed, and no unaudited production HTTP client bypasses that boundary;
2. runtime, persistence, logs, errors, fixtures, reports, and tracked files do not disclose
   reusable Secret material; credential storage uses the established AEAD boundary;
3. public API requests require a valid Client Key and only reach a route granted to its active
   Access Group, while management endpoints separately require management authentication,
   loopback/private policy, and CSRF protection for state changes;
4. disabled/revoked/expired credential paths, auth parsing failures, ungranted routes, and
   management cross-origin or missing-CSRF paths deny rather than fall through;
5. the locked Rust and admin-UI dependency sets pass their respective policy/advisory checks, with
   direct exceptions recorded rather than silently ignored; and
6. a deterministic, reviewable SBOM names only locked dependencies and records the generation
   tool, target/platform scope, artifact hash, and audit timestamp without copying credentials or
   local paths.

## Implementation and validation sequence

1. Inventory every network-client construction, EgressPolicy admission call, public/management
   route registration, credential/Client Key comparison, Secret serialization/debug/logging seam,
   CI supply-chain step, Cargo lockfile and UI lockfile. This stage is source-only and must not
   use provider, OAuth, server, database, browser-session, proxy, or production credentials.
2. Run targeted adversarial regressions for egress admission, redirect/DNS pinning, auth and
   access-grant denial, management auth/CORS/CSRF, encrypted Secret storage and redaction. Add
   narrow regression coverage only for a concrete audit gap; do not change product semantics for
   speculative hardening.
3. Run tracked/staged Secret checks plus their scanner regression. Audit Cargo with the pinned
   `cargo-deny` and `cargo-audit` tools, audit the committed UI lockfile without lifecycle
   scripts, and record exact tool versions and the advisory snapshot date.
4. Generate a CycloneDX JSON SBOM from the locked Cargo workspace into a tracked evidence path;
   validate its JSON, require a non-empty component list, reject absolute local paths and
   credential-shaped values, and record a SHA-256 digest. The report must distinguish the Rust
   SBOM from the separately audited npm lockfile rather than claiming a combined graph.
5. Write the Security Report with PASS/FAIL/DEFERRED findings, residual risks and evidence
   commands. Run the required local gate, independently review enforcement and report claims,
   mark P11-05 `LOCAL_PASS_PENDING_PHASE_GATE` only when every required boundary passes, then
   commit. P11-06 must not begin before that review and commit.

## Explicitly out of scope

No endpoint probing, provider/OAuth/API Key use, server configuration or deployment, penetration
testing against public targets, credential rotation, operating-system hardening, P11-06 recovery
drills, P11-07 migration exercises, or P11-08 release packaging. The audit does not certify a
production deployment; P12 remains responsible for real-server controls and the 72-hour Canary.
