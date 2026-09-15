# Prism daily-workspace alignment production receipt

## Release

Prism daily-workspace alignment hardening is deployed to the existing Oracle
CPAR service at revision `0ff3b82e715ce750f05a7116b444f375edfd60dd`.

| Check | Evidence |
| --- | --- |
| Signed artifacts | GitHub Actions run `34948527229` completed for native x86_64 and ARM64. Each build completed its binary, SBOM, OCI smoke, manifest, Sigstore signing, receipt verification, and artifact upload steps. |
| Oracle artifact | The native ARM64 artifact manifest and Sigstore bundle were independently verified locally before upload. The staged and running binary SHA-256 was `44ffbb59932c9d961d76f2518af31f30e0c6958467e5c87fc9f1e533a016475a`. |
| Cutover | The `current` release link moved atomically from `7988bffd60d4c40cfa45c7cee39126b7d441043b` to `0ff3b82e715ce750f05a7116b444f375edfd60dd`. The CPAR systemd unit returned active and `/healthz` passed before the 12-second automatic rollback deadline. |
| Public application | EgoLite opened the existing public administrator login page. Same-origin GETs confirmed the embedded index, `main.js`, `vendor.js`, and `index.css`; CSP excludes `unsafe-inline` and `unsafe-eval`. |

The prior schema-28 binary `7988bff` remains installed as the immediate
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

- 307 Prism frontend tests passed.
- Type check, production build, double-build SPA gate, and embedded-management
  Rust integration test passed.
- The real local gateway/EgoLite flow verified historical exact-range filter
  preservation, intentional relative-preset replacement, request timing
  summaries, provider filtered-empty recovery, and mobile/desktop overflow
  boundaries.
- The management client regression suite verified that a stalled GET reaches its
  deadline, releases the read slot, and lets a following read proceed; a
  cancelled queued read settles without dispatch. Writes remain unqueued and
  are never replayed.
