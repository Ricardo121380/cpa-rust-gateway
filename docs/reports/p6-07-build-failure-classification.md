# P6-07 Build-specific failure classification

| Field | Value |
|---|---|
| Status | `DONE` |
| Scope | Bounded HTTP status/signal to Gateway error and permitted state-action mapping |
| References | `E06-E12`; [BC-PROVIDER-004](../contracts/BC-PROVIDER-004-grok-build-runtime-continuity.md) |

The decoder contributes only safe status/signal syntax. P6-07 maps invalid OAuth or `401` to reauthorization; a `403` without independent account evidence to Egress rejection; confirmed forbidden to account scope; Free usage to one quota window; account/provider `429` separately; and `408`/`5xx` to Provider cooldown. Transient and egress paths never permanently disable a refreshed Credential.

Verification: the error matrix test covers 401, two 403 evidence states, Free quota 429, account 429, and provider 503. It contains no raw response body or live account state.
