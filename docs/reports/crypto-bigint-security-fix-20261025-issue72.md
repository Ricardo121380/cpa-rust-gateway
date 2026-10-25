# RUSTSEC-2026-0387: compatible P-256 dependency upgrade

Date: 2026-10-25 (Asia/Shanghai). Branch: `codex/72-usage-query-export`.
Security-change baseline: `d3dda4baae42256f8a8aea62534bb1c52126cf25`.

**Status: local security fix, affected tests, Full and pre-commit review PASS.**

This separately authorized security change preserves the complete local
[#72 implementation](issue-72-usage-query-export-20261025.md). Its historical Full/audit failures
remain bound to their original code commits and logs. No advisory exemption is added.

## Upgrade decision

[RUSTSEC-2026-0387](https://rustsec.org/advisories/RUSTSEC-2026-0387.html) concerns possible
secret-dependent timing in intended constant-time arithmetic. Its patched range starts at
`crypto-bigint 0.7.6`; it does not establish an exploit against CPAR's particular DPoP use.

The crates.io manifests were checked on 2026-10-25, including renamed dependency entries:

| Path | Constraint and consequence |
|---|---|
| Existing `p256 0.13.2 → elliptic-curve 0.13.8 → crypto-bigint 0.5.5` | `p256` requires `elliptic-curve ^0.13.1`; that generation requires `crypto-bigint ^0.5`, which excludes the patched range |
| Minimum compatible stable entry: `p256 0.14.0 → elliptic-curve 0.14.1 → crypto-bigint 0.7.6` | `p256` requires `elliptic-curve ^0.14.1`; `elliptic-curve 0.14.1` requires bigint `^0.7.5`, allowing the first patched release |
| Selected lock: `p256 0.14.0 → elliptic-curve 0.14.2 → crypto-bigint 0.7.8` | Normal stable resolution after targeted `cargo update -p p256 --precise 0.14.0`; `elliptic-curve 0.14.2` requires bigint `^0.7.7` |

Registry sources: [p256](https://index.crates.io/p2/56/p256),
[elliptic-curve](https://index.crates.io/el/li/elliptic-curve),
[crypto-bigint](https://index.crates.io/cr/yp/crypto-bigint).
The selected releases are not yanked. The selected elliptic-curve/bigint MSRV is Rust 1.87,
within the repository's pinned Rust 1.97.1. The compatible patch resolution also includes
the bigint 0.7.7 arithmetic corrections and 0.7.8 const-evaluation improvement documented in its
[upstream changelog](https://docs.rs/crate/crypto-bigint/0.7.8/source/CHANGELOG.md).
No unrelated top-level dependency was upgraded. Lockfile identity additions/removals are confined
to the P-256 dependency closure: 19 additions and 13 removals, with all 353 shared package
name/version/source/checksum identities unchanged (366 packages before, 372 after).

## Code and shared interfaces

- Enable `p256/getrandom` and remove the now-unused direct `rand_core 0.6` dependency from
  the workspace, provider manifest and exact crate-boundary allowlist. Other consumers' transitive
  `rand_core` versions remain intact.
- `GrokConsoleDpopSession::generate_key` now returns `Result<SigningKey, GrokConsoleDpopError>`
  using `Generate::try_generate()`. Entropy failure becomes the existing `Randomness` category
  instead of a panic. Both inference bootstrap/refresh call sites map it to their existing
  egress error; native quota observation maps it to existing `Unavailable`.
- The P-256 public key API changes from `to_encoded_point(false)` to `to_sec1_point(false)`.
  Console still sends an EC/P-256 public JWK, signs ES256 with SHA-256 and RFC 6979, and exports
  the fixed-width 64-byte `r || s` JWT signature. No private key is exported or persisted.
- The direct public Rust signature above changes; all in-repository callers are updated.
  Management HTTP/OpenAPI, database schema (38), migrations, Usage/ledger semantics and #72
  snapshot/export interfaces are unchanged. No price refresh or quota-history feature is added.

This is RustCrypto API adaptation, not a new Go-provider behavior port. Existing Console v3.1.1
wire/session behavior and [BC-SEC-008](../contracts/BC-SEC-008-provider-specific-egress-health-recovery.md)
remain the contract. No new ADR or protocol behavior is introduced.

## Validation

Evidence root: this worktree's ignored `output/issue-72/rustsec-2026-0387/`.
Compilation uses its own `target`; `TMPDIR` is the evidence root's `tmp` directory.
Tests use synthetic inputs/local services and do not enable ignored real-Provider probes.

| Check | Result |
|---|---|
| `cargo test --locked -p provider-grok --all-features` | PASS: 257 passed, 0 failed, 4 ignored; `logs/provider-grok.log` |
| `proof_contains_dpop_claims_and_fixed_width_signature` | PASS: token-exchange JWK has exactly four public fields, 32-byte x/y, independent SEC1 reconstruction, canonical thumbprint and both POST/GET proofs; exported JWK verifies signatures and rejects modified claims |
| `es256_matches_rfc6979_p256_sha256_vector` | PASS: exact public RFC 6979 Appendix A.2.5 SHA-256/`sample` signature bytes, anchoring algorithm/encoding independently of sign/verify self-consistency |
| Existing DPoP cache/session regression | PASS within provider suite |
| `native_account_usage::tests` | PASS: all five existing tests within the Full workspace run; Console zero/unknown handling and other channel parsers remain covered |
| `cargo test --locked --workspace --all-features` | PASS: 1677 passed, 0 failed, 21 ignored within Full; `logs/full.log` |
| `cargo deny check` | PASS: advisories, bans, licenses and sources; `logs/deny.log` |
| `cargo audit` | PASS: 372 locked dependencies, no vulnerabilities or warnings; `logs/audit.log` |
| `scripts/check.sh full` | PASS: all 44 steps, exit 0; `full-report.md` and `logs/full.log`; completed `2026-10-25T05:45:19Z` |
| Standards and spec review | PASS: no actionable findings; independent dependency-closure verification completed the initial spec review's tooling-related evidence gap |

The explicit RFC fixture is a published test vector, not a real credential. Tests neither contact
the URLs embedded in DPoP claims nor assert real-Provider acceptance. No timing/side-channel
measurement, remote CI, release, deployment or real-account qualification is claimed.

Review compared the frozen staged increment against `d3dda4baae42256f8a8aea62534bb1c52126cf25`
before any security-fix commit. Standards found zero documented-rule violations and zero actionable
heuristic findings. Spec found zero behavioral deviations; its first dependency enumeration script
could not import the host Python's unavailable `tomllib`, so a separate read-only check using
standard-library parsing verified the official version constraints and complete lockfile closure.
The primary agent retains responsibility for final runtime validation and result-only documentation.
Full also passed the stock serve envelope, Agent multi-turn/stream regression, Clippy, source and
crate-boundary policies, document/contract references, tracked secret scan, tool versions and both
supply-chain checks. Subsequent changes only record results in this report, the historical #72 report
and the current roadmap; their links, contract references, staged secrets and whitespace are checked
again before committing. The Full script's whitespace step checks HEAD, so the staged security
increment is additionally checked explicitly with `git diff --cached --check`.

## Duplicate dependency warning assessment

All 15 `cargo deny` duplicate warning families were assessed from the current lock/reverse graph.
These are warnings under the existing policy, not advisory exemptions. `deny.toml` is unchanged.

| Warning family | Assessment |
|---|---|
| `hmac`, `sha2` | Newly duplicated by RFC6979/P-256's new digest generation; existing gateway authentication, hashing and provider HMAC consumers retain their declared 0.12/0.10 APIs. Upgrading those is unnecessary for this advisory |
| `block-buffer`, `crypto-common`, `digest` | Existing 0.10/0.11 digest split already required by SHA-1 versus Argon2/Blake2; the P-256 closure now uses the newer generation too. Both generations remain required |
| `cpufeatures` | Existing 0.2 users (Argon2, AEAD, SHA-256 0.10) and 0.3 users (SHA-1/new SHA-256, rand's ChaCha20) remain version-compatible within their respective APIs |
| `chacha20` | Existing AEAD 0.9 and rand 0.10 implementations are separate consumers; no AEAD or RNG policy change is required |
| `getrandom`, `rand_core` | Existing OS randomness/RNG generations are required by ring, password-hash, test RNGs and newer rand. P-256 now uses its own supported 0.10 trait generation with getrandom 0.4; no mismatched RNG is passed across generations |
| `http`, `socket2` | Existing Actix versus Hyper/wreq/runtime version boundaries; unrelated to elliptic-curve arithmetic |
| `shlex`, `syn` | Existing build/procedural-macro dependency version boundaries (bindgen/cc, macro ecosystem); no runtime crypto interface is shared |
| `thiserror`, `thiserror-impl` | Existing tokio-socks 1.x versus telemetry/quinn/tungstenite 2.x dependencies; error derive generations remain independently required |

The complete lockfile can also retain target/optional entries not active in the host graph.
No dependency duplicates or advisories have been suppressed to make the gate pass.
