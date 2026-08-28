# ADR-0053 Kiro failure-owner classification

| Field | Value |
|---|---|
| Status | Accepted |
| Task | `P7-08` |
| Depends on | `P7-01`; `P7-06` |
| Contract | [BC-PROVIDER-011](../contracts/BC-PROVIDER-011-kiro-failure-owner-classification.md) |

Kiro's network failures, authentication loss, account entitlement loss, missing model, quota
window exhaustion, and ordinary service load have different remediation owners. Treating every
403 as a Credential ban or every 429 as quota would destroy healthy credentials and collapse the
dynamic per-Credential catalog introduced in P7-06.

The Kiro provider accepts only safe, pre-classified observations: network phase or HTTP status
plus independently established signal. It returns a `GatewayError` and a single permitted action,
but does not mutate state. DNS/connect/TLS/first-byte failures are egress-local; post-semantic
interruption is stream-local. `401` requires reauthorization. An unknown `403` is only
`EgressRejected`; account forbiddance needs independent account evidence. An unavailable model
cools the model only, a concrete quota signal cools one quota window, an account 429 cools one
account, and an ordinary 429 cools the Provider. `408` and `5xx` are Provider-transient.

No raw status body, Header, URL, model ID, Credential, account identifier, reset time, or retry
is retained here. A future transport may derive safe signals under its own bounded parsing rule;
it cannot use this function to turn arbitrary body text into an account mutation. P7-09 owns the
bounded real adapter/differential evidence, while a later runtime owner must apply actions through
its own revision-aware state transition boundary.
