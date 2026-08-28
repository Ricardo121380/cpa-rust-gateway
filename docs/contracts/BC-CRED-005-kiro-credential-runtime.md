# BC-CRED-005 Kiro credential and refresh runtime

| Field | Value |
|---|---|
| Task | `P7-01` |
| ADR | [ADR-0046](../adr/ADR-0046-kiro-credential-runtime-boundary.md) |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |

## Required behavior

| Concern | Requirement |
|---|---|
| Families | Social requires an OAuth token pair and absolute expiry; Enterprise additionally requires client ID, client secret, and auth Region; API key requires `ksk_` and is not refreshable. Mixed or unknown shapes fail closed. |
| Secret boundary | Tokens, API keys, and client secrets are zeroized and absent from `Debug`. Persisted bytes are authenticated AEAD ciphertext with AAD bound to the exact Credential ID. |
| Refresh | A Social/Enterprise refresh is one injected transport call with bounded strict response parsing. A same-Credential concurrent caller waits at most 30 seconds and returns the new revision or an explicit reload state; a different Credential is never serialized behind it. |
| Revision | Initial runtime state is revision 0. CAS advances exactly one revision. A refresh leader whose source revision changes before completion returns `ConcurrentCredentialStateChanged` and cannot overwrite the newer value. |
| Regions | Enterprise `auth_region` is syntactically validated and carried only for refresh. API Region and IDE/CLI inference policy are deliberately absent until P7-02. |

## Exclusions

This contract does not create HTTP/TLS clients, load a local credential cache, persist a runtime
row, infer an API Region, choose an endpoint, or execute inference. Those boundaries belong to
later P7 Tasks.

## Corresponding tests

- `three_credential_families_are_strictly_distinct_and_redacted`
- `import_rejects_duplicate_mixed_expired_and_malformed_credential_shapes`
- `refresh_is_kind_specific_redacted_and_never_refreshes_an_api_key`
- `sealed_credential_is_aead_bound_to_its_exact_credential_id`
- `same_credential_expiry_singleflights_and_followers_observe_the_new_revision`
- `different_credentials_refresh_without_waiting_for_each_other`
- `old_refresh_winner_cannot_overwrite_a_newer_cas_credential`
