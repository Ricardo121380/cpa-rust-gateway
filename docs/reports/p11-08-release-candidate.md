# `v0.1.0-alpha.1` Release Candidate ledger

| Field | Candidate value |
|---|---|
| Candidate label | `v0.1.0-alpha.1` — an evidence label only; no Git tag or published artifact exists. |
| Cargo workspace version | `0.1.0` in the root workspace manifest. This task deliberately does not rewrite it. |
| Intended artifact owner | P12-01: pinned build, checksum, SBOM, signature and tag/release decision. |
| Rust / edition | `rustc 1.97.1`, edition `2024`. |
| Workspace | 21 packages; `gateway` is a local management CLI, not a configured HTTP server process. |
| Control-plane schema | `9`, from `CURRENT_SCHEMA_VERSION`. |
| License / publication | `Apache-2.0`; Cargo publication is disabled. |
| Candidate status | `LOCAL_PASS_PENDING_PHASE_GATE` — all P11 task evidence is local; G11 and its one GitHub Delivery Gate are still required. |

## Candidate inventory

| Area | Included, source-backed capability | Evidence / boundary |
|---|---|---|
| Public protocols | Authenticated OpenAI Responses and Anthropic Messages adapters over the Canonical lifecycle, bounded streams, Tool and Reasoning semantics. | P1, P3 and P5 reports; P11-01 differential fixtures and P11-02 fault matrix. |
| Routing and upstream isolation | Immutable RouteSnapshot, Client-Key/Access-Group admission, endpoint-format isolation, bounded credentials, DNS-pinned allowlisted egress, quota/circuit state and exact retry ownership. | P2–P4 reports and P11-05 security audit. |
| Provider families | OpenAI-compatible, Grok Build, Grok Web and Kiro implementation paths remain separated by Provider/Credential/runtime state. | P6/P7/P8/P9 task reports; external-validation differences below remain material. |
| Management control plane | Protected resource, routing, status, lifecycle, encrypted backup/empty-target restore and embedded management UI components. | P10 reports; no deployment/listener claim. |
| Release hardening | Offline differential fixtures, loopback fault matrix, benchmark comparator, ≥10h local synthetic soak evidence, security audit/SBOM, shutdown/recovery drills and upgrade/rollback rehearsal. | P11-01 through P11-07 reports. |

## Production-default posture

These are **safe library/entry-point defaults**, not a deployable P12 configuration file.

| Surface | Safe default / invariant | P12 action before use |
|---|---|---|
| Process and listener | `apps/gateway` exposes only the transport-free `gateway admin` CLI; it binds no inference or management listener. | Supply explicit service unit, data directory, listener ownership, resource limits and health/readiness behavior. |
| Public inference | `ResponsesHttpState` requires a Client-Key authenticator; no unauthenticated public route is constructed by its normal API. | Provision Client Keys and a published RouteSnapshot; retain per-client access scope. |
| Management network/browser | `ManagementNetworkPolicy` defaults to loopback-only; `ManagementBrowserPolicy` rejects browser Origins unless an exact same-origin plus independent CSRF token is configured. | Keep a private listener and configure only a deliberate trusted UI origin/CSRF secret. |
| Egress | An `EgressPolicy` cannot be created with empty schemes, hosts or ports. DNS/IP/redirect policy must be explicit. | Declare narrow per-Endpoint host/CIDR/port/scheme allowlists; do not inherit proxy or ambient endpoint state. |
| Event persistence/export | A normal HTTP state uses `NoopGatewayEventSink`; durable SQLite writer, telemetry exporters and their paths are explicit composition choices. | Configure bounded queues, writer task, telemetry destination, retention and monitoring separately. |
| Stream and queues | Default Canonical stream capacity is 8; default Required/Diagnostic event capacities are 1024/128 with hard queue cap 8192. Full queues produce explicit non-blocking outcomes. | Size and observe these limits under the P12 Canary; do not turn them into unbounded buffers. |
| Database and backup | No data path, Backup Key or Master Key is discovered by this candidate ledger. Encrypted restore creates only an absent target. | Set owned data/backup/key directories, verify key permissions and rehearse the P12 backup/rollback procedure. |

## Known differences, deferrals and release blockers

| ID | Truthful state | Required next owner |
|---|---|---|
| `RC-EXT-001` | P7 Kiro native path has local evidence, but its Kiro-RS `--bare` live tuple remains `DEFERRED_EXTERNAL_AUTH`; do not claim current upstream-account success. | Final external-authentication package after a user-supplied/re-authenticated Kiro account. |
| `RC-EXT-002` | P8 Official has local safety/compatibility evidence, but P8-07 has no Official API key and remains deferred with P7-09. | Final external-authentication package after an explicitly authorized Official key/probe. |
| `RC-REL-001` | P11-04's local synthetic loopback receipt ran 10h13m and was user-stopped, so it truthfully remains `INCOMPLETE` while accepted under `CR-P11-04-001`; it is not a real 24h/production soak. | P12-10 real deployment 72h Canary. |
| `RC-DEP-001` | No release artifact, checksum, signature, Docker image, systemd/Caddy/Cloudflare configuration, server database, listener bind, provider route, or production credential is part of this candidate. | P12-01 through P12-10. |
| `RC-GATE-001` | Local P11 evidence alone cannot establish GitHub Required checks or a deployment. | G11 phase closeout, one P11 GitHub Delivery Gate, then P12. |

## Handoff and acceptance boundary

This ledger is fit to enter the P11 phase-closeout review only. It is not permission to publish a
release or deploy a gateway. P12 begins only after G11 is accepted and its GitHub Delivery Gate
passes; it must then create a fresh artifact identity and record its exact revision, checksum,
signature, SBOM and service configuration.

## Verification and review

| Check | Result |
|---|---|
| Workspace/package/schema/default source cross-check | PASS — values above come from the root `Cargo.toml`, `gateway-store`, `apps/gateway`, `gateway-http-actix`, `gateway-observability`, `gateway-upstream` and the cited P11 reports. |
| `./scripts/check.sh docs` | PASS after candidate, README and report-index update. |
| Secret review | PASS — no key, endpoint, account, absolute path, raw Provider request or production configuration value is recorded. |

Focused review confirms that `v0.1.0-alpha.1` is explicitly not confused with the current Cargo
version or a published build, every default is presented as a source-level posture rather than a
deployed setting, and all known deferred external/production work is carried forward to its
correct owner.
