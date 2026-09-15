# Prism daily-workspace alignment production receipt

## Release

Prism daily-workspace alignment is deployed to the existing Oracle CPAR
service at revision `7988bffd60d4c40cfa45c7cee39126b7d441043b`.

| Check | Evidence |
| --- | --- |
| Signed artifacts | GitHub Actions run `34946952551` completed for native x86_64 and ARM64. Each build completed its binary, SBOM, OCI smoke, manifest, Sigstore signing, receipt verification, and artifact upload steps. |
| Oracle artifact | The native ARM64 artifact manifest and Sigstore bundle were independently verified locally before upload. The staged and running binary SHA-256 was `3ed539bd644ccea30edee6ae27483cf84d7a5883547fd5ba071db7e8b4f9f0b3`. |
| Cutover | The `current` release link moved atomically from `e77bcb5b79a81fac1db33aec2a1912eb5c9e50fe` to `7988bffd60d4c40cfa45c7cee39126b7d441043b`. The CPAR systemd unit returned active and `/healthz` passed in 910 ms. |
| Public application | EgoLite opened the existing public administrator login page. Same-origin GETs confirmed the embedded index, `main.js`, `vendor.js`, and `index.css`; CSP excludes `unsafe-inline` and `unsafe-eval`. |

The previous schema-28 binary `e77bcb5` remains installed as the immediate
rollback target. This frontend/client-only release does not migrate control or
administrator stores.

## Boundaries retained

- Existing administrator, accounts, active configuration, request history, and
  billing records were retained.
- No DNS, Caddy, firewall, Autoreg, credential, model alias, or provider
  configuration change was made.
- No real Provider metadata, probe, authorization, or inference request was
  made. Local acceptance used only an owned loopback TLS mock.
- Public browser verification stopped at the administrator login screen; no
  production administrator password was entered by this release process.

## Local validation before deployment

- 304 Prism frontend tests passed.
- Type check, production build, double-build SPA gate, and embedded-management
  Rust integration test passed.
- The real local gateway/EgoLite flow verified 7-day deep links, exact-range
  filter preservation, explicit 24-hour replacement, request timing summaries,
  provider filtered-empty recovery, and mobile/desktop overflow boundaries.
