# ADR-0046 Kiro Credential runtime boundary

| Field | Value |
|---|---|
| Status | Accepted |
| Task | `P7-01` |
| Contract | [BC-CRED-005](../contracts/BC-CRED-005-kiro-credential-runtime.md) |

## Decision

`provider-kiro` represents Social OAuth, Enterprise/IdC OAuth, and CLI `ksk_` API keys as
separate credential variants. Strict, bounded JSON is explicitly supplied by a caller; no Kiro
cache, environment variable, database, or network source is read by this boundary. Secrets are
zeroized, diagnostic-safe, and can be sealed by `gateway-store::SecretStore` with AAD bound to the
exact `CredentialId`.

Social and Enterprise refreshes use a caller-injected transport. The in-memory runtime coordinator
is per Credential ID, bounded to a 30-second same-key wait, and revision guarded. A late leader
cannot overwrite a newer CAS value; a follower never starts a second refresh. CLI API keys never
refresh. Durable database persistence and endpoint/API Region selection remain later P7 work.

## Consequences

- One account's refresh cannot block another account's refresh.
- Ciphertext cannot be opened under another Credential ID.
- P7-02 owns IDE/CLI URL, API Region, headers, Origin, and request policy; P7-01 does not infer
  them from a credential.
- The transport request keeps token/client-secret fields private to `provider-kiro`; the public
  injected interface receives only the secret-safe request category and Enterprise auth Region.
