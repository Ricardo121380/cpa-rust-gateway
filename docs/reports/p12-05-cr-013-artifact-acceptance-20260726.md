# P12-05 CR-013 exact artifact acceptance

| Field | Result |
|---|---|
| Plan / Task | `v1.61` / `P12-05` (`IN_PROGRESS`) |
| Approved repair revision | `49f8c0f3eb6326d3f2ed6cc612ec8ffd10915938` |
| Private Actions workflow | [`release-artifact` run 30185145888](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/30185145888), completed successfully for the same revision |
| Private artifact | ID `8626803209`; `p12-01-release-artifact-49f8c0f3eb6326d3f2ed6cc612ec8ffd10915938` |
| Build contract | Rust `1.97.1`, target `x86_64-unknown-linux-gnu`, version `0.1.0` |
| Download integrity | Archive SHA-256 `436b7af9d6e987d46d35162fdc2814ca47c332b2afc2d47cc3c47f7e98ab30ff`; ZIP integrity check passed. |
| Download extraction | Exactly the required nine regular files were present, with no extras. |
| Manifest SHA-256 | `367eb2ff4f0dea16b3f69ce6a780e7f504cc233865cb9e73c4401dadf49388d5` |
| Gateway ELF SHA-256 | `e6e13f3a3e69eb7d0ede11815c9b526bf8d6e33a96c5343307b8faf20eb260f4` |
| SBOM SHA-256 | `775be2ff37af311ebc1580a46215e9e6a40ea01216cb8bcec4376c12e0796478` |
| OCI archive SHA-256 | `40514d78d5693c64bfe326a1e804f415dcde308b2b234bdc676379edafd821bc` |
| Repository verification | `p12-release-artifact.rb verify --require-signature --require-receipt` and `inspect-oci` both passed. |
| Independent signature verification | Cosign `verify-blob` using the artifact bundle, the GitHub Actions OIDC issuer, and the pinned `release-artifact.yml` workflow identity returned `Verified OK`. |

## Verification scope

The independent local verifier checked the manifest payload hashes, build metadata, receipt
cross-links, ELF architecture and embedded release metadata, SBOM value policy, and OCI
layout/digests. The extracted gateway is a 64-bit little-endian `x86-64` GNU/Linux executable.
The workflow receipt revision, manifest digest, SBOM digest, keyless-signing type, and workflow
run all matched the downloaded artifact and the completed private GitHub workflow.

The Sigstore verification used only the public GitHub workflow identity and issuer stored in
`signing-identity.json`. No Provider endpoint, Bearer, OAuth field, model, request, response, or
account identity was read into this receipt. The artifact remains private to GitHub Actions and
the disposable local verification directory.

## Review conclusion

`CR-P12-05-013` is revision-bound to the reviewed P12-only Messages lifecycle repair and
satisfies the required local provenance, structure, content, OCI, and keyless-signature checks.
No server path, systemd unit, Staging database, credential envelope, Client Key, route, listener,
Provider request, or incumbent CPA state changed during this acceptance.

The only permitted next action is one fresh isolated-Staging transaction using this exact artifact:
readiness/listener checks, loopback Models, and one non-streaming Messages request whose response
must expose the repaired `end_turn` and numeric usage lifecycle. The preimage must be restored
regardless of outcome. Tool, Explain, direct probes, P12-06, and public exposure remain
prohibited.

## Subsequent controlled execution

That one permitted transaction completed successfully and restored the Staging preimage. Its
value-free result is recorded in the [CR-013 isolated Staging receipt]
(evidence/p12-05-cr-013-staging-receipt-20260726.md). This artifact acceptance remains a
provenance record; it does not authorize the still-pending Tool, Explain, P12-06, or public
exposure steps.
