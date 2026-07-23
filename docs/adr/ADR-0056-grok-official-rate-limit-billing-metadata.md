# ADR-0056: Grok Official rate-limit and billing metadata boundary

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-23` |
| Task | `P8-03` |
| Matrix / Contract | `C01`、`C31`、`C33`、`E11`、`G16`、`G26`; [BC-PROVIDER-014](../contracts/BC-PROVIDER-014-grok-official-rate-limit-billing-metadata.md) |

## Context

Official xAI response headers can communicate rate-limit capacity and reset timing, but they are
not an account balance, a billing plan, a price, or permission to change scheduler state. The
existing router accepts only sanitized quota observations; it deliberately does not parse raw HTTP
headers. Grok Build's billing parser/state cannot be reused because `grok.official`, `grok.build`,
and `grok.web` require independent quota/billing parser and runtime namespaces.

P8-03 therefore has to make the narrow wire-to-safe-metadata conversion. P8-05 later owns exact
Official state application/isolation, while P8-06 and live E2E can confirm real header behavior.

## Decision

1. Parse only case-insensitive `x-ratelimit-limit/remaining/reset-{requests,tokens}` triplets and
   delta-seconds `Retry-After`. All other header names and values are ignored and never retained.
2. A rate-limit resource is valid only when all three members occur exactly once, are bounded
   unsigned decimal/duration values, `remaining <= limit`, and reset is positive and no more than
   24 hours. A duplicate known header, partial group, invalid UTF-8, HTTP-date Retry-After,
   zero/overflowed delay, or impossible window fails as a safe provider protocol error.
3. Extend the shared raw upstream response only with a read-only iterator over all values for one
   requested Header name. It makes duplicate detection possible; it does not log, classify, or
   retain a value. The Official production transport converts only the fixed allow-list into the
   redacted metadata object before passing the response boundary.
4. Project already canonical provider-reported token counters into `GrokOfficialBillingMetadata`.
   Do not infer a plan, account balance, price, currency, charge, or quota from response usage or
   rate-limit headers.
5. P8-03 performs no state mutation. P8-05 will map valid metadata to a source-labelled exact
   Official target and will prove it cannot affect Build/Web state. Status/credential/health/retry
   classification remains a later explicit boundary.

## Consequences

- Header parsing remains bounded, deterministic, chunk-independent, and safe to attach to the
  existing injected Official transport without adding a real HTTP call.
- An HTTP intermediary cannot make ambiguity appear authoritative by duplicating a known header.
- `Retry-After` date form is not converted without a clock; the eventual runtime may make a
  separately specified date/clock decision rather than silently trusting wall time here.
- Billing and rate limiting retain different meanings: token usage is an observed count, not
  financial or scheduling authority.

## Alternatives considered

- Reuse Grok Build billing/quota state: rejected by `C31` and source-specific semantics.
- Preserve arbitrary headers or an unrestricted HeaderMap: rejected because raw values can leak
  and unbounded header vocabulary is not a quota contract.
- Accept partial windows or choose one duplicate: rejected because missing/ambiguous quota data
  could erroneously cool or unblock an account.
- Parse HTTP-date Retry-After with local clock: rejected because this pure parser has no trusted
  observed-time input.
- Call an xAI billing endpoint: rejected because no fixed Official billing endpoint/account
  authorization is established and `CR-P7-G7-001` forbids real Official E2E at this stage.

## Validation and rollback

Synthetic fixtures cover case-insensitive complete request/token windows, retry delay, redaction,
partial/duplicate/invalid/impossible evidence, and usage-only billing metadata. Formatting,
Clippy, full workspace tests, source/crate boundaries, document links, Secret checks, dependency
policy, and RustSec audit must pass locally.

Rollback removes this metadata module, the duplicate-header iterator, tests, ADR, contract,
report, and traceability entry. It changes no persisted quota, account state, route, API key,
server, proxy/TUN setting, production traffic, or real endpoint.
