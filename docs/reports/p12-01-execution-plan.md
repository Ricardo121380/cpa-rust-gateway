# P12-01 Release artifact execution plan

| Field | Value |
|---|---|
| Plan version | `v1.46` |
| Task | `P12-01` |
| Status | `DONE` — the signed private artifact from workflow run `30112894068` passed independent downloaded-artifact, signature, Rekor, manifest, SBOM, ELF and OCI verification. |
| Branch | `codex/p12-deployment` |
| Task Card | Produce a fresh, revision-bound Linux `x86_64` gateway binary and OCI image archive, a redacted CycloneDX SBOM, SHA-256 manifest, detached signature and machine-verifiable receipt. Artifacts remain private CI outputs until a later P12 task authorizes a server deployment. |
| References | [P12 plan](../06-development-plan.md#18-p12---服务器部署与灰度), [P11 candidate ledger](p11-08-release-candidate.md), [G11](g11-gate-report.md), [environment baseline](environment-baseline.md), [quality gates](../quality-gates.md) |
| Acceptance | [P12-01 release artifact report](p12-01-release-artifact.md) |

## Observed starting boundary

- The passed P11 closeout target is `074beb9`; P12 must mint a new artifact identity rather than
  relabel the earlier `v0.1.0-alpha.1` evidence label.
- Production is Linux `x86_64`, while the local machine is macOS `arm64`. The server toolchain is
  older than the locked Rust `1.97.1`, so P12 must consume a CI-built Linux artifact rather than
  compile on the server.
- The repository currently has no Dockerfile, release workflow, OCI registry destination, signing
  policy, Cosign/Syft executable, or configured local container daemon. `cargo-cyclonedx` and
  SHA-256 tooling are available locally.

## Required acceptance

1. A pinned Linux CI build produces an `x86_64-unknown-linux-gnu` release binary, the formal OCI
   image archive and an ephemeral Docker-compatible export used only for runtime smoke. The formal
   payload carries the exact Git revision, locked Rust version and architecture; the image runs as
   a non-root user and exposes no configured listener, Secret, Provider route, credential or server
   path.
2. A deterministic artifact manifest names every released file by filename, byte size and SHA-256.
   A verification script rejects duplicate names, unexpected files, malformed digests and any
   mismatch before a later server task may consume it.
3. A redacted CycloneDX SBOM is generated from the locked workspace. It contains no checkout path,
   `file://` qualifier, credential, endpoint, request/response value, or absolute local path.
4. A detached signature and its verification identity cover the manifest digest, not an ambiguous
   mutable tag. The receipt records the artifact revision, digests, SBOM digest, signature type and
   verifier identity without recording private key material.
5. Building, SBOM generation, checksumming, signing and verification are separate fail-closed
   steps. An unsigned, locally fabricated, wrong-architecture or changed post-signing artifact
   cannot be presented as release-ready.

## Approved signing authority

The user approved GitHub Actions OIDC keyless Sigstore signing for this task on 2026-07-24. The
detached signature covers the immutable SHA-256 manifest; its identity is restricted to this
repository's pinned `release-artifact.yml` workflow/ref, and Sigstore's public transparency log is
explicitly authorized. No signing private key is created, stored, logged or copied to the server.

The OCI archive remains a private GitHub Actions artifact. This authorization does **not** allow a
GitHub Release, release tag, registry push, server login or deployment. A separately run
`cosign verify-blob` with the recorded GitHub OIDC issuer and workflow identity must pass before
the artifact receipt may claim `verified`.

## Implementation and validation sequence

1. Add a pinned release-build workflow and scripts for a locked Linux binary, OCI archive,
   path-redacted SBOM, SHA-256 manifest and fail-closed verification. The workflow uploads only
   private CI artifacts; it does not create a GitHub Release, tag, registry push, listener or
   server change.
2. Add a minimal multi-stage Dockerfile with a non-root runtime and immutable revision/version
   labels. Build the OCI archive in Linux CI and inspect its architecture, entrypoint, user and
   digest without starting a service.
3. Run unit/negative tests for manifest and SBOM sanitization, the applicable local gate and an
   independent review. The approved workflow may then mint one private signed artifact.
4. After the chosen signing flow emits a detached manifest signature, verify it in a separate
   fail-closed step before writing the value-free receipt, then independently verify the downloaded
   artifact before moving on to P12-02. Any failed signature, digest, architecture, SBOM-redaction
   or image-policy check freezes P12.

## Explicitly out of scope

No release tag, GitHub Release, public artifact, registry push, server login, Docker daemon change,
systemd/Caddy/Cloudflare mutation, listener bind, backup, Provider/OAuth/API-key call, route
configuration, credential provisioning, Canary traffic or deployment is authorized by this plan.
