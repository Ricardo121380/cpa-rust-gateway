# P6-08 Grok Build clean-room differential report

| Field | Value |
|---|---|
| Method | Read-only schema/behavior comparison; no source copy, credential export, API key, request body, response text, or server mutation |
| Reference | Deployed `grok2api` image `ghcr.io/chenyme/grok2api:v3.0.6`; frozen CPA/grok2api/Sub2API behavior references already recorded by P6-03 |
| Status | `DONE` — the remediated P6 Delivery Gate passed |

## Structural correspondence

| Reference behavior class | P6 representation | Intentional difference |
|---|---|---|
| Account Billing snapshot and model capabilities | `grok_build_billing_profiles` plus `grok_build_model_catalog`, exact Credential scope | P6 keeps a compact typed plan/source model and does not copy reference history payloads. |
| Multiple quota windows, reset, recovery/block state | `grok_build_quota_windows` with source/confidence and explicit kinds | P6 records observations only; probing and scheduler recovery remain shared-runtime work. |
| Prompt-cache affinity and breakability | derived HMAC cache identity, `grok_build_cache_affinities`, `grok_build_affinity_breaks` | P6 refuses raw cache keys instead of retaining a reference-style raw key/value field. |
| Response to account ownership | `grok_build_response_ownership` | P6 rejects owner mismatch rather than trying another account. |
| Reasoning replay key/state | `grok_build_reasoning_replay`, AEAD and associated data | P6 stores no plaintext replay or generic session map. |
| OAuth refresh/account health | P6-01/P6-02 revisioned AEAD Credential runtime and P6-07 actions | Provider-specific failure action is separate from error parsing and no `5xx` permanently disables an account. |

## Evidence and boundary

A privileged server-local SQLite inspection emitted only the container image, relevant table names, and column names. It confirmed separate Billing, capability, quota-window, quota-recovery, credential, provider-account, audit, and response-ownership structures. It did not select rows or render encrypted material, email, model, identifier, header, body, or API key.

The existing P6-03 isolated server proxy reference remains a separate fact: its single Build-routed OpenAI-compatible Responses generation completed through grok2api. It does not convert the direct official CLI request's earlier `unattributed` 4xx into a direct success. Under the user-approved P6 continuation CR, that uncertainty is retained as a documented operational limitation rather than a reason to omit durable isolation and error safeguards.

No reference implementation source was copied. The differential uses observable structure and the documented behavior above, while this repository's types, migrations, HMAC derivation, AEAD replay, and fail-closed ownership rules are independently implemented.

Reopened review passed: the crate-boundary policy declares the Adapter-to-Router fixture link as a
test-only edge, and no `include_str!`, path/git dependency, or source import references the
clean-room projects. The remediated P6 Delivery Gate subsequently passed, completing this Task.
