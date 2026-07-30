# P12-01 Release artifact acceptance

| Field | Result |
|---|---|
| Task | `P12-01` |
| Status | `DONE` |
| Accepted revision | `ecabe04e46b950af56bb19dda779ddf4d37b4f91` |
| Release workflow | [run 30112894068](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/30112894068), `SUCCESS`, about 4m01s |
| Private artifact | `p12-01-release-artifact-ecabe04e46b950af56bb19dda779ddf4d37b4f91`, artifact ID `8604337375`, ZIP bytes `34702833`, expires `2026-08-07T17:28:52Z` |
| Signing boundary | GitHub Actions OIDC keyless Sigstore signature over the immutable manifest; public Rekor entry; no private signing key |
| Deployment boundary | No registry push, GitHub Release/tag, server login, listener, systemd, Caddy/Cloudflare or deployment change |

## Accepted artifact

The downloaded artifact contains exactly the five manifested payloads plus the manifest, detached
signature, Sigstore bundle and receipt. The repository verifier passed with
`--require-signature --require-receipt` against the accepted merge revision, Rust `1.97.1`, target
`x86_64-unknown-linux-gnu` and version `0.1.0`.

| Payload | Bytes | SHA-256 |
|---|---:|---|
| `gateway-build-metadata.json` | 259 | `256804c9f7c6ce3938c0e5b7cf6afb744b40091b632f0f835a81618ce79eede5` |
| `gateway-image.oci.tar` | 33361408 | `d87fd896991e5f2824e875596de91a3280eabbb533655e6df08d6daa8d265d20` |
| `gateway-sbom.cdx.json` | 159464 | `bcbbe894a61d53d1dc1ab200c1b3b9a01bbd778d7450148ad064e88e644af839` |
| `gateway-x86_64-unknown-linux-gnu` | 3153304 | `d4a6e18da042035bf26a0bf36de81a80f19af18c8df0fc2fc475ab5fa1d7fb98` |
| `signing-identity.json` | 273 | `08e550c78923b9be15c18b22955599c1f694584a78449c4bf58e80b538906a9b` |

The manifest SHA-256 is
`98f95ac1d2401c009e6e18d7447559d1ac8943ad0c542a654b3b551fb4fa3049`.
The receipt records the same revision, manifest and SBOM digests and exactly eight pre-receipt
files with `signing.type=sigstore-keyless` and `verification=verified`.

## Independent verification

| Check | Evidence | Result |
|---|---|---|
| Binary identity | `file` reports ELF 64-bit LSB PIE, x86-64, GNU/Linux; the verifier confirms embedded revision, Rust version and target markers. | PASS |
| Build metadata | Revision `ecabe04e...`, Rust `1.97.1`, target `x86_64-unknown-linux-gnu`, version `0.1.0`. | PASS |
| Manifest | Exact deterministic filenames, positive byte sizes and SHA-256 values all match downloaded files; unexpected files fail closed. | PASS |
| SBOM | CycloneDX `1.5`, 205 components, exact target triple; the recursive verifier rejects local paths, URLs and forbidden sensitive keys. | PASS |
| OCI | Linux/amd64, user `65532:65532`, entrypoint `/usr/local/bin/gateway`, exact revision/version/toolchain/target labels and no exposed listener; unsafe paths, symlinks and root user remain negative tests. | PASS |
| Runtime smoke | A Docker-compatible export from the same Buildx build loaded successfully and ran `--help` with `--network none --read-only --user 65532:65532`. It is runner-temporary and is not uploaded or signed as a release payload. | PASS |
| Blob signature | OpenSSL verification of the detached ECDSA/SHA-256 signature against the Fulcio leaf certificate returned `Verified OK`; the detached signature equals the bundle signature. | PASS |
| Certificate identity | SAN is `https://github.com/Ricardo121380/cpa-rust-gateway/.github/workflows/release-artifact.yml@refs/heads/main`; OIDC issuer extension is `https://token.actions.githubusercontent.com`; revision extension is the accepted merge SHA. | PASS |
| Transparency log | [Rekor log index 2236353888](https://rekor.sigstore.dev/api/v1/log/entries?logIndex=2236353888), UUID `108e9186e8c5677aba1086e05fc471c087f17b4024f40cedb9fcf558b6edb2058640512263067299`, integrated `2026-07-24T17:28:47Z`; public body/log ID/index/time match the bundle and an inclusion proof is present. | PASS |
| Repository verifier | `scripts/p12-release-artifact.rb verify ... --require-signature --require-receipt` and `inspect-oci` both pass on the downloaded private artifact. | PASS |

The workflow also ran its own pinned `cosign verify-blob` identity/issuer check before it was
allowed to write the receipt or upload the artifact. The independent download review did not rely
on the receipt's `verified` label alone: it recalculated payload/receipt digests, verified the blob
signature, inspected the certificate and queried the public Rekor entry.

## Failure and repair review

Three real workflow failures were retained as evidence and repaired in order. None reached signing,
the transparency log or artifact upload.

| Run | Terminal failure | Repair |
|---|---|---|
| [30101393023](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/30101393023) | The default Docker Buildx driver did not support the OCI exporter. | PR [#2](https://github.com/Ricardo121380/cpa-rust-gateway/pull/2) creates and explicitly selects a `docker-container` builder. |
| [30110198819](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/30110198819) | The verifier rejected Buildx's legitimate `blobs/` directory entry. | PR [#3](https://github.com/Ricardo121380/cpa-rust-gateway/pull/3) accepts safe ordinary directories while rejecting traversal, duplicates, symlinks and special tar types. |
| [30111585968](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/30111585968) | Docker Engine `docker load` cannot import an OCI layout archive. | PR [#4](https://github.com/Ricardo121380/cpa-rust-gateway/pull/4) emits an ephemeral Docker-compatible smoke archive from the same build while preserving OCI as the formal artifact. |

Each repair passed focused checks, the complete local Fast gate, review, and its PR's Classify,
Fast, Full supply-chain and Required checks before the next single manual release run.

## Review and next boundary

Review found no release-blocking issue. The final run has a non-blocking GitHub annotation that
some pinned actions still declare Node.js 20 and are currently forced by the runner onto Node.js 24;
the pinned actions completed successfully, but a future maintenance task should update them before
GitHub removes compatibility.

P12-01 authorizes only this private artifact and its public Sigstore log entry. It does not prove a
server installation. P12-02 is now the sole `IN_PROGRESS` task and owns the systemd unit, read-only
Secret boundary, data/log directories and resource limits. Server backup remains P12-03, and no
Staging process may start before both P12-02 and P12-03 satisfy their plan dependencies.

## Amendment: dual-target release matrix (`CR-P12-01-002`, 2026-07-27)

The acceptance recorded above remains the accepted evidence for its `x86_64-unknown-linux-gnu`
artifact; the digests and run links in this report are not rewritten. `CR-P12-01-002` extends the
release pipeline to a second target because the confirmed production host is `aarch64`, which cannot
execute the binary accepted above.

| Aspect | Before | After |
|---|---|---|
| Targets | `x86_64-unknown-linux-gnu` | plus `aarch64-unknown-linux-gnu` |
| Runners | `ubuntu-24.04` | one runner per target architecture (`ubuntu-24.04`, `ubuntu-24.04-arm`) |
| Build mode | native | native for both; no cross toolchain and no qemu, so the no-network read-only non-root `--help` execution still runs the binary that gets signed |
| Artifact sets | one | one per target, each with its own SBOM, manifest, keyless signature and receipt; no multi-architecture index and no cross-target manifest |
| Verifier | single target constant | closed target table deriving the binary name, ELF `e_machine` (62 / 183), OCI architecture (`amd64` / `arm64`) and pinned base digest; unknown targets fail closed |
| Base image | one amd64 digest written into the Dockerfile | one pinned digest per architecture, both members of the `debian:bookworm-slim` index `sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818`; supplied as a build argument, so the verifier re-establishes the pin from the image's own `base.name` label and the workflow checker asserts the matrix matches the verifier's table |

The previously pinned base digest `sha256:63a496b5…` is a single-architecture amd64 manifest rather
than a multi-architecture index, so the arm64 job pins `sha256:9b672946…` explicitly; resolving the
amd64 digest on an arm64 runner would fail.

Local evidence for the amendment: the verifier's positive and negative paths run once per target,
including new cross-architecture rejections (each target refuses the other's ELF machine, the
other's OCI architecture, the other's binary filename, the other's pinned base digest, an unpinned
`debian:bookworm-slim`, and a fully signed set checked against the wrong target), plus fail-closed
rejection of targets outside the table. The workflow checker was exercised against three injected
regressions — a base digest the verifier does not enforce, an arm64 target moved onto an x86_64
runner, and an added `setup-qemu-action` step — and rejected each. A real
`aarch64-unknown-linux-gnu` CycloneDX SBOM (205 components, correct target triple) was generated by
the production generator and passed the full verifier end to end. The remote dual-job run is a
separate CI-only step and is not claimed here.

While generating that SBOM, an unrelated defect surfaced and was fixed: both SBOM generators cleaned
`cargo-cyclonedx` raw output through a `{apps,crates}/**` glob, so the `tests/differential`
workspace member added in P12-06 left an untracked raw SBOM containing local filesystem paths in the
checkout. Both generators now derive the cleanup set from `cargo metadata`, which covers every
member including future ones. The frozen P11-05 SBOM evidence file was not rewritten.
