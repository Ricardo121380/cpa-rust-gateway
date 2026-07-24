# P11-08 Release Candidate execution plan

| Field | Value |
|---|---|
| Plan version | `v1.45` |
| Task | `P11-08` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Branch | `codex/p11-release-hardening` |
| Task Card | Produce a source-backed `v0.1.0-alpha.1` candidate ledger: build/schema provenance, shipped capability inventory, safe default posture, known external/deployment differences, explicit P12 handoff, and no false claim of a published artifact. |
| References | [P11 plan](../06-development-plan.md#17-p11---发布加固), [P11 security audit](p11-05-security-audit.md), [P11 recovery report](p11-06-recovery-report.md), [P11 upgrade/rollback report](p11-07-upgrade-rollback.md), [P12 plan](../06-development-plan.md#18-p12---服务器部署与灰度) |

## Required acceptance

1. The candidate ledger distinguishes its human release-candidate label (`v0.1.0-alpha.1`) from
   the current Cargo workspace package version (`0.1.0`), and explicitly states that P12-01 owns
   the built, checksummed, signed/tagged artifact. It creates no tag, binary, Docker image, or
   deployment.
2. The inventory records the authoritative Rust/toolchain, schema, workspace, security and
   compatibility evidence already reviewed by P11 without copying credentials, endpoints, raw
   Provider traffic, absolute checkout paths, or transient runtime data.
3. The production-default section is source-backed and fail-closed: no application listener is
   configured by default; management remains loopback/Origin-deny by default; inference requires
   Client-Key authentication; egress requires a configured allowlist; event persistence/export is
   opt-in; and deployment-specific paths/keys/listeners remain P12 configuration.
4. The known-difference list truthfully carries forward deferred Kiro/Official external
   authentication, the local synthetic soak limitation, and every P12 deployment/Canary
   responsibility. README and report index point to the candidate ledger.

## Implementation and validation sequence

1. Read the workspace manifest, Store schema/version, binary entry point, management default
   policies, egress/event defaults, and P11 evidence. Record only stable values with their source
   locations.
2. Write the candidate ledger and update stale README/project/report-index status. Do not change
   package version, release script, server configuration, credential, or external state.
3. Run docs/plan/link/Secret checks, review every inventory/default/difference claim against its
   source, mark P11-08 `LOCAL_PASS_PENDING_PHASE_GATE`, and commit. Completed: docs, plan, link,
   whitespace and Secret checks pass; the candidate remains the plan's active item while the
   single P11 phase closeout and GitHub Delivery Gate run. Do not start P12 before they pass.

## Explicitly out of scope

No release tag, GitHub release, package version change, build artifact, image, signature,
checksum, systemd/Caddy/Cloudflare configuration, listener bind, server backup, Provider/OAuth/API
key call, account change, or real deployment is authorized. P12 owns all of those operations.
