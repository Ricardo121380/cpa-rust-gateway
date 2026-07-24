# P7-01 Kiro Credential runtime report

| Field | Value |
|---|---|
| Task | `P7-01` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Scope | Strict Social/Enterprise/`ksk_` credentials, AEAD sealing, injected refresh, per-Credential singleflight and revision CAS |
| References | `C36-C38`, `E25-E26`; [ADR-0046](../adr/ADR-0046-kiro-credential-runtime-boundary.md); [BC-CRED-005](../contracts/BC-CRED-005-kiro-credential-runtime.md) |

`provider-kiro` now rejects mixed, duplicate, expired, oversized, malformed-region, or wrong-kind
credential input. It does not discover a source path or perform I/O. OAuth credential bytes are
sealed with exact-Credential AAD; cross-Credential decryption fails. The injected refresh request
does not expose its form secrets in diagnostics, and API keys cannot refresh.

The runtime coordinator is in-process and deliberately not a second token cache: its revision/CAS
boundary prevents a stale refresh winner from replacing a newer control-plane value. Same-ID callers
singleflight with a bounded wait; different IDs are independent. Durable row ownership and endpoint
policy remain outside this task.

## Verification and review

| Check | Result |
|---|---|
| `cargo test --locked -p provider-kiro --test p7_01_credentials` | PASS; seven strict-family, redaction, AEAD, refresh, same-key singleflight, different-key independence, and stale-winner regressions |
| `cargo clippy --locked -p provider-kiro --all-targets --all-features -- -D warnings` | PASS |
| `ruby scripts/check-crate-boundaries.rb` | PASS; Kiro's narrow `serde`/`serde_json`/`zeroize`/`gateway-store` edge is documented |
| `./scripts/check.sh full` | PASS; workspace format, Clippy/tests, source policy, Secret scan, crate boundaries, documents, dependency policy, and RustSec audit |

Review focus: no secret value is included in a debug record, API keys cannot take the refresh path,
followers cannot create a second same-Credential exchange, and a CAS mutation wins over a late
refresh result.
