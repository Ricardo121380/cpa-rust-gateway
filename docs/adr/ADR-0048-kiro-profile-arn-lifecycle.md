# ADR-0048 Kiro profile ARN lifecycle

| Field | Value |
|---|---|
| Status | Accepted |
| Task | `P7-03` |
| Contract | [BC-PROVIDER-006](../contracts/BC-PROVIDER-006-kiro-profile-arn-lifecycle.md) |

`provider-kiro` validates every `profileArn`, queries Enterprise through an injected port, uses a
region-family fallback only on failed/invalid lookup, and injects only a resolved value at a JSON
request root. API keys omit it. P7-01's combined Social/Builder family lacks a Social-provider
discriminator, so it is explicitly treated as the frozen Builder profile rather than guessing an
identity. Audit projections contain source, Region, and presence only—never an ARN or credential.
