# P12-05 CR-005 exact artifact acceptance

| Field | Result |
|---|---|
| Plan / Task | `v1.53` / `P12-05` (`IN_PROGRESS`) |
| Approved replacement revision | `2aef274e624c9bb35547623b5383f14731fae515` |
| Private Actions workflow | [`release-artifact` run 30166446304](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/30166446304), completed successfully for the same revision |
| Private artifact | ID `8621799024`; `p12-01-release-artifact-2aef274e624c9bb35547623b5383f14731fae515` |
| Build contract | Rust `1.97.1`, target `x86_64-unknown-linux-gnu`, version `0.1.0` |
| Download extraction | Exactly the required nine regular files were present, with no extras. |
| Manifest SHA-256 | `e9286fe4b02f4481decafc7f33d66924382f164491a7cbc9576284dbfb96fc53` |
| Gateway ELF SHA-256 | `df67a2cd62807acaba780863c37802ab6330b7cd87df8834be31a8fcbaebb30e` |
| SBOM SHA-256 | `775be2ff37af311ebc1580a46215e9e6a40ea01216cb8bcec4376c12e0796478` |
| OCI archive SHA-256 | `e436487de97ca46ade1ab47d02e4c915235beb63ff0de6ba6d61e364c4b89e23` |
| Repository verification | `p12-release-artifact.rb verify --require-signature --require-receipt` and `inspect-oci` both passed. |
| Independent signature verification | Homebrew Cosign `v3.1.2` ran `verify-blob` against the downloaded bundle using the artifact's GitHub Actions OIDC issuer and pinned workflow identity; result: `Verified OK`. |

## Verification scope

The independent verifier checked the manifest payload hashes, build metadata, receipt
cross-links, ELF architecture and embedded release metadata, SBOM value policy, and OCI
layout/digests.  The ELF check identified a 64-bit little-endian `x86-64` GNU/Linux executable.
The receipt revision, manifest digest, SBOM digest, keyless signing type, and workflow run all
matched the downloaded artifact and the completed GitHub workflow metadata.

The Sigstore verification used only the public workflow identity and issuer stored in
`signing-identity.json`; no Provider endpoint, Bearer, OAuth field, model, request, response, or
account identity was read into this receipt.  The artifact remains private to GitHub Actions and
the local disposable verification directory.

## Review conclusion

`CR-P12-05-005`'s replacement artifact is revision-bound to the reviewed P12-only Messages
output-limit repair and satisfies the required local provenance, structure, content, and
keyless-signature checks.  No server path, systemd unit, Staging database, credential envelope,
Client Key, route, listener, Provider request, or incumbent CPA state was changed by this
acceptance.

P12-05 may now enter its separately ordered isolated-Staging preimage and temporary
singleton-graph sequence.  P12-06 remains pending.
