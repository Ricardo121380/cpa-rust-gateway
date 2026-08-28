# BC-PROVIDER-011 Kiro network, account, model, quota, and rate-limit classification

| Field | Value |
|---|---|
| Task | `P7-08` |
| ADR | [ADR-0053](../adr/ADR-0053-kiro-failure-owner-classification.md) |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |

| Observation | Canonical error / scope | Permitted action |
|---|---|---|
| DNS, connect, TLS, or pre-first-byte timeout | `EgressUnavailable` / `Egress` | Rebuild only egress |
| Stream interruption after semantic output | `StreamTruncated` / `Stream` | Terminate stream; no transparent retry |
| `401` or independent unauthorized signal | `CredentialUnauthorized` / `Credential` | Require explicit reauthorization |
| Unknown `403` | `EgressRejected` / `Egress` | None |
| Independently confirmed account forbidden | `CredentialForbidden` / `Account` | Mark only that account forbidden |
| Model unavailable | `RouteNotFound` / `Model` | Cool only model capability |
| Concrete quota exhausted | `CredentialQuotaExceeded` / `QuotaWindow` | Cool only quota window |
| Account-specific `429` | `ProviderRateLimited` / `Account` | Cool only account |
| Ordinary/provider `429`, `408`, or `5xx` | `ProviderRateLimited` or `ProviderTransient` / `Provider` | Cool Provider only |

Signals override a generic status only when they come from a separately bounded parser or
observation. The classifier stores neither raw body/header/URL nor Credentials, account or model
identities; it does not send a probe, refresh/retry, select a new endpoint, or mutate runtime
state.
