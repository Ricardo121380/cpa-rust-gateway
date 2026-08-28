# BC-SEC-003 Grok Web Statsig cache and signer SSRF boundary

| Field | Value |
|---|---|
| Contract | `BC-SEC-003` |
| Task | `P9-05`; production composition `P12-10I-14` |
| ADR | [ADR-0065](../adr/ADR-0065-grok-web-statsig-cache-ssrf-boundary.md) |
| Matrix | `C29`、`D28`、`E27-E29` |
| Status | P9 cache boundary accepted; P12-10I-14 production composition `IN_PROGRESS_ARTIFACT_E2E` |
| Domain | Exact Statsig signature cache plus fixed Web environment/signer transport and P2 admission |

## Preconditions and bounds

1. A signature cache key has an upper-case method of at most 16 bytes, an absolute printable path of at most 2048 bytes with no query/fragment/backslash, and an opaque printable environment version of at most 128 bytes.
2. A signature is printable, has no header controls, and is at most 16 KiB. Cache capacity is 1–256; expiry is strictly later than caller time.
3. Signer admission receives an explicit P2 `EgressPolicy` and resolver. Initial signer URLs must parse as HTTPS before P2 admission. Redirects always receive P2 re-admission.

## Required behavior

| Concern | Required behavior |
|---|---|
| Cache isolation | A signature is returned only for the same method/path/environment key. Expired reads remove only their exact key; no unexpired different key is evicted to make capacity. |
| 403 remediation | `invalidate_after_403` removes only the supplied exact key and returns whether it existed. It has no bulk, prefix, account, or cross-environment invalidation operation. |
| Secret safety | Signatures are zeroizing and redact in `Debug`. Paths/environment, signer URLs, DNS answers, and resolver diagnostics are not retained in cache/signature errors. |
| SSRF admission | Initial and redirect targets use P2 exact scheme/host/port/DNS/CIDR/redirect validation. A signer target not using HTTPS fails. The wrapped target provides no public raw URL or send operation. |
| Production environment | P12-10I-14 fetches only the fixed Web `/index` target with the exact credential-bound Cookie/User-Agent and Chrome transport, extracts only bounded `grok-site-verification`, and signs only `POST /rest/app-chat/conversations/new`. |
| Signer response | The fixed HTTPS signer response must contain exactly one `x-statsig-id`; Base64 decoding must yield exactly 70 bytes. Sign failure refreshes the environment and retries signing once before failing closed. |
| Runtime cache/retry | One Endpoint owns one 1h singleflight cache. A pre-start conversation 403 conditionally invalidates only the signature actually sent and retries once; an older concurrent 403 cannot delete a newer signature. No retry occurs after the first Canonical Event. |
| Ownership | P9-06 owns quota and P9-07 owns account-versus-egress attribution. P12-10I-14 owns the production signer/session transport and its explicit isolated live E2E. |

## Corresponding tests

- `signature_cache_is_exact_key_isolated_and_a_403_invalidates_only_one_entry`
- `cache_reclaims_only_expired_entries_and_rejects_unsafe_keys_values_and_capacity`
- `signer_requires_https_exact_allowlist_and_full_redirect_readmission_without_sending`
- `cache_is_singleflight_and_a_403_refreshes_only_the_current_signature`
- `pre_start_403_refreshes_statsig_once_then_projects_the_live_stream`
- `production_meta_and_signature_shapes_match_the_frozen_reference`
