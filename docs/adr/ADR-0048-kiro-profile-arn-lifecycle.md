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

## CPAR C amendment — 2026-09-30

Completed CLI model discovery may supply a validated, exact-API-region profile for the leased
OAuth credential. The serving generation retains it under endpoint, credential ID and revision,
with validity bounded by credential expiry and one hour. Inference uses that observed profile
only for the same current revision. An expired or mismatched observation fails closed; absence
of an observation retains the reviewed Social/Enterprise resolution above. API keys neither
use OAuth catalog discovery nor receive a profile. A discovery from a retired generation or
rotated revision cannot publish profile metadata for the new material.

This in-memory metadata is not a new account store, public management contract, or proof of a
successful real Kiro call. See the separately scoped [C evidence](../reports/cpar-batch-c-20260930.md).
