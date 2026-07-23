# P6-06 Build Response Ownership and Reasoning Replay

| Field | Value |
|---|---|
| Status | `DONE` — the remediated P6 Delivery Gate passed |
| Scope | Exact continuation ownership plus encrypted tenant/model/session replay state |
| References | `D27`, `F07`, `F17`; [BC-PROVIDER-004](../contracts/BC-PROVIDER-004-grok-build-runtime-continuity.md) |

`response_ownership` is idempotent only for the exact Credential, upstream response id, and expiry. A different selected Credential returns `OwnershipCredentialMismatch`; another tenant does not see the response. Replay uses the existing AEAD SecretStore with all namespace fields as associated data, suppresses identical writes, never logs plaintext, and supports exact clear after a confirmed no-replay result.

Verification: the reopened synthetic continuity test again proves mismatch/missing ownership errors, cross-tenant replay absence, AEAD ciphertext non-retention of test plaintext, deduplication, and clearing. The remediated P6 Delivery Gate subsequently passed, completing this Task.
