# P11-05 Security audit report

| Field | Value |
|---|---|
| Plan version | `v1.45` |
| Task | `P11-05` |
| Audit date | `2026-07-24` |
| Scope | SSRF/egress, Secret handling, public and management authentication, Access Group authorization, dependency/supply-chain policy, and release-candidate SBOM. |
| Result | PASS — Full local gate and focused review passed; `LOCAL_PASS_PENDING_PHASE_GATE`. |
| References | [P11 plan](../06-development-plan.md#17-p11---发布加固), [EgressPolicy report](p2-09-egress-policy-ssrf-admission.md), [Secret-store report](p2-03-aead-secret-store.md), [security policy](../../SECURITY.md) |

## Finding disposition

| ID | Severity | Finding | Disposition |
|---|---|---|---|
| `P11-05-F01` | Moderate (local metadata disclosure) | `cargo-cyclonedx` emitted workspace `bom-ref` values containing the Mac's absolute checkout path and `file://` download qualifiers. No credential was exposed, but the raw artifact was not publishable. | FIXED. [`generate-p11-05-sbom.rb`](../../scripts/generate-p11-05-sbom.rb) rewrites every component and dependency reference to a stable Cargo PURL, rejects remaining local/file references, and deletes all raw intermediate SBOMs. [`test-generate-p11-05-sbom.sh`](../../scripts/test-generate-p11-05-sbom.sh) proves deterministic output and JSON/local-reference invariants. |

No open Critical, High, Moderate, or Low finding remains in the audited source and locked dependency
sets. The finding above is retained for traceability; it was discovered and remediated during this
audit, before the evidence artifact was staged.

## Control review

| Area | Source and adversarial evidence | Result |
|---|---|---|
| SSRF and DNS rebinding | [`EgressPolicy::admit_url`](../../crates/gateway-upstream/src/egress_policy.rs) validates exact scheme/Host/port, resolves immediately before the attempt, rejects empty/mixed/special DNS answers, and returns a pinned address set. [`UpstreamClientPool`](../../crates/gateway-upstream/src/upstream_client.rs) accepts only `AdmittedEgressTarget`, disables ambient proxy/DNS/redirect/retry behavior, keys pooling by the admitted addresses, and supplies those addresses to Reqwest. A production-source scan found the sole `reqwest::Client` construction in this pool. | PASS |
| Redirect and proxy escape | Redirects are disabled at the client and policy defaults; policy redirect handling fully re-admits each Location with a finite hop limit. Only local-DNS `socks5` is accepted; remote-DNS/HTTP proxy schemes and proxy user-info are rejected/redacted. `gateway-upstream` tests cover default metadata/private/IPv6 denial, mixed DNS, re-resolution, redirect re-admission, no hidden retry, pinned direct/SOCKS transport, and timeout boundaries. | PASS |
| Secret storage and logs | [`SecretStore`](../../crates/gateway-store/src/secret_store.rs) uses versioned AEAD envelopes, external strict key files, redacted `Debug`, and zeroization. [`ClientKeyService`](../../crates/gateway-auth/src/client_key.rs) retains HMAC digests, performs constant-time checks, zeroizes secret material, and rejects symlink Pepper files. [`log_safety`](../../crates/gateway-observability/src/log_safety.rs) defaults to no body retention and redacts sampled sensitive content. | PASS |
| Tracked Secret control | [`SECURITY.md`](../../SECURITY.md) forbids credentials and raw traces. [`secret-scan.sh`](../../scripts/secret-scan.sh) rejects credential paths and recognizable literals without printing values; its regression and whole-index scan passed. | PASS |
| Public authentication and authorization | Every model/Responses/Messages/count endpoint parses exactly one `Authorization: Bearer` or `x-api-key` value before request decoding. Snapshot authentication fails closed and resolves the active Access Group; [`SnapshotClientKeyView`](../../crates/gateway-router/src/route_snapshot.rs) carries copied granted Route IDs and filters the visible public-model view. Invalid, duplicate, disabled, revoked, expired, unknown, or ungranted requests reject before Provider execution. | PASS |
| Management security | `/admin` is protected by a separate management header/key, fail-closed trusted peer policy, browser-origin policy, and state-changing CSRF check in [`management_security`](../../crates/gateway-http-actix/src/management_security.rs). Regression covers absent/duplicate/wrong key, forwarded-header spoofing, public/link-local/private address classes, hostile origin, and missing CSRF; denial is a value-free `404` with `no-store`. | PASS |
| Rust supply chain | `deny.toml` allows only crates.io and approved licenses, denies yanked crates/wildcards/unknown sources, and has no advisory ignore or source exception. Pinned `cargo-deny 0.20.2` and `cargo-audit 0.22.2` passed. `cargo deny` reports four non-fatal, non-exempt duplicate-version warnings (`getrandom`, `http`, `socket2`, `syn`) from independently versioned upstream dependency families; it reports advisories, bans, licenses, and sources all OK. | PASS with monitored warnings |
| Admin UI supply chain | The committed lockfile contains only the root package and `typescript 5.9.3` development dependency. `npm audit --package-lock-only --json` reported 0 vulnerabilities across all severities, without lifecycle scripts. | PASS |
| Rust SBOM | [`p11-05-rust-sbom.cdx.json`](evidence/p11-05-rust-sbom.cdx.json) is a CycloneDX 1.5 `gateway` runtime SBOM for `x86_64-unknown-linux-gnu`, generated from the locked 21-package workspace with all features. It contains 205 components/206 dependency entries, has no local paths or file URIs, and has SHA-256 `060341d73b46596d482733f30e6606346f21fbf38270f02252149b4cab50798b`. The artifact's stable timestamp derives from the last dependency-source commit (`2026-07-23T22:44:44Z`), not the workstation clock. | PASS |

## Validation evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-upstream -p gateway-auth` | PASS — 38 tests for SSRF/DNS/redirect/pinning/transport and Client Key lifecycle/redaction. |
| `cargo test --locked -p gateway-http-actix --test p10_02_management_security --test p10_05_management_routing` | PASS — 5 management auth/network/origin/CSRF/Secret-redaction tests. |
| `cargo test --locked -p gateway-store secret_store` | PASS — 8 Secret-store AEAD/key-load/rotation/redaction tests. |
| `cargo test --locked -p gateway-http-actix --lib snapshot_responses_rejects_a_non_visible_model_before_executor_start` and `cargo test --locked -p gateway-router public_model_view_filters_access_groups_and_requires_hard_eligible_candidates` | PASS — non-visible model and Access Group denial stay before execution. |
| `./scripts/test-install-quality-tools.sh`, `./scripts/check-ci-workflow.rb`, and `./scripts/test-generate-p11-05-sbom.sh` | PASS — the SBOM tool is pinned, cacheable, deterministic, valid JSON, and path-free. |
| `./scripts/secret-scan.sh --all`, `./scripts/check.sh supply-chain`, and `npm audit --package-lock-only --json` | PASS — tracked Secret scan, Cargo policy/RustSec audit, and npm advisory audit. |
| `./scripts/check.sh full` | PASS — shell/CI/plan guards, 21-package format/Clippy/test workspace, source/crate boundaries, 309 Markdown links, tracked Secret scan, pinned tools, Cargo policy, and RustSec audit. |

## Residual boundaries

This is a source/fixture/lockfile audit. It does not authorize public endpoint probing,
credential/OAuth use, server penetration testing, or operating-system configuration changes. P12
must still apply production bind/Secret/file-permission controls and perform the real 72-hour
Canary. The Cargo duplicate-version warnings remain visible because they are not allowlisted or
ignored; a future dependency update should remove them where upstream compatibility permits.

## Focused review

PASS. The release artifact retains 207 unique component references and 206 dependency entries;
every dependency reference resolves, while the sanitizer removes only local `path+file`/`file://`
provenance. Its regression independently regenerates the artifact twice and rejects absolute local
paths. `cargo-cyclonedx` is installed and version-checked only in the existing Full
supply-chain path, so the Fast gate remains unchanged; the Full cache key includes the pinned
version and its binary. No raw CycloneDX output remains in the workspace, and `.playwright-cli/`
is unrelated pre-existing untracked content excluded from this Task's commit.
