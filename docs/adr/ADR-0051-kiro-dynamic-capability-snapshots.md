# ADR-0051 Kiro per-Credential dynamic capability snapshots

| Field | Value |
|---|---|
| Status | Accepted |
| Task | `P7-06` |
| Depends on | `P4-02`; `P7-01` |
| Contract | [BC-PROVIDER-009](../contracts/BC-PROVIDER-009-kiro-dynamic-capability-snapshots.md) |

Kiro's available-model and subscription information is Credential-specific. A source failure for
one account is not evidence that its last listed model disappeared, and it cannot invalidate a
different account's entitlement. The reference was inspected read-only: the upstream model list
is Credential-scoped; subscription information is a separate observation. No reference source,
credential, request, response body, or private model mapping is copied into this repository.

`provider-kiro` therefore accepts a paired, injected observation per exact `CredentialId` and
retains an immutable combined model/subscription success at that exact key. It adopts P4-02's
explicit Fresh/Stale/Expired timing and last-success semantics. A later success is atomic for that
Credential, time-monotonic, versioned, and never overwrites another Credential. Probe failures do
not persist error data; they may reuse only their own non-expired last success in the aggregate.

The stored subscription projection is deliberately coarse: `Free`, recognized `Paid`, or
`Unknown`, plus supported/unsupported/unknown overage. Raw titles are parsed transiently and
discarded. Models retain only normalized source IDs and optional source token limits. The union is
sorted and deduplicated by exact source ID; it never manufactures a `-thinking` alias. Conflicting
per-Credential token limits become unknown in the union rather than advertising an unsafe larger
limit.

This decision has no HTTP client, endpoint fallback, OAuth refresh, retries, account/quota
classification, route publication, public model endpoint, Tool/Thinking conversion, or real
request. P7-07 owns Kiro semantic mappings, P7-08 owns network/account/error taxonomy, and P7-09
owns the bounded real adapter and differential evidence.
