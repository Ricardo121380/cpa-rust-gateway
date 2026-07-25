# P12-05 CR-004 exact artifact acceptance

| Field | Result |
|---|---|
| Plan / Task | `v1.52` / `P12-05` (`IN_PROGRESS`) |
| Approved replacement revision | `7a5dead43b257f487e526051acf544578841e6ae` |
| Private Actions workflow | [`release-artifact` run 30161380336](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/30161380336), completed successfully for the same revision |
| Private artifact | ID `8620365683`; name `p12-01-release-artifact-7a5dead43b257f487e526051acf544578841e6ae` |
| Build contract | Rust `1.97.1`, target `x86_64-unknown-linux-gnu`, version `0.1.0` |
| Download integrity | A `38,189,455`-byte ZIP passed `unzip -t`; extraction contained exactly the required nine regular files and no extras. |
| Manifest SHA-256 | `0ff8378fc3e46ff1dc6ac80326e9aa39e4c7ccfce6a4b4b7165a7d07b683135b` |
| Gateway ELF SHA-256 | `2788e5a2ea6351fe23290f5aa6a0b08ced1d2fbb2a45643bb68bbe5cd3dd45c2` |
| SBOM SHA-256 | `775be2ff37af311ebc1580a46215e9e6a40ea01216cb8bcec4376c12e0796478` |
| OCI archive SHA-256 | `05dc60a6d1138d9799be641ad3b3e950ac1f5aa8d9ab48ddc602f376a5f05506` |
| Repository verification | `p12-release-artifact.rb verify --require-signature --require-receipt` and `inspect-oci` both passed. |
| Independent signature verification | Homebrew Cosign `v3.1.2` ran `verify-blob` against the downloaded bundle using the artifact's GitHub Actions OIDC issuer and pinned workflow identity; result: `Verified OK`. |

## Verification scope

The repository verifier independently checked the manifest's payload hashes, build metadata,
receipt cross-links, ELF architecture and embedded release metadata, SBOM value policy, and OCI
layout/digests.  The ELF check identified a 64-bit little-endian `x86-64` GNU/Linux executable.
The receipt revision, manifest digest, SBOM digest, keyless signing type, and workflow run all
matched the downloaded artifact and the successful GitHub workflow metadata.

The Sigstore verification used only the public workflow identity and issuer stored in
`signing-identity.json`; no Provider endpoint, Bearer, OAuth field, model, request, response, or
account identity was read into this receipt.  The artifact itself remains private to GitHub Actions
and the local disposable verification directory.

## Review conclusion

`CR-P12-05-004`'s replacement artifact is revision-bound to the reviewed P12-only compatibility
repair and satisfies the required local provenance, structure, content, and keyless-signature
checks.  No server path, systemd unit, Staging database, credential envelope, Client Key, route,
listener, Provider request, or incumbent CPA state was changed by this acceptance.

P12-05 may now proceed to its separately ordered isolated-Staging preimage and temporary
singleton-graph sequence.  P12-06 remains pending.
