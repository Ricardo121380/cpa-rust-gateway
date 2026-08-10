# Autoreg Oracle → CPAR production provider-binding canary

- Date: 2026-08-10 (Asia/Shanghai)
- Change boundary: `CR-P12-AUTOREG-MIGRATE-001`
- Environment: Oracle Singapore ARM64 production CPAR data plane, real public base URL
- Public base URL: `https://cpar.example.invalid` (the legacy CPA URL is out of scope)
- Credential handling: one eligible Autoreg Build OAuth record was streamed through a root-only
  in-memory normalizer and CPAR AEAD import. No token, account identity, endpoint, model value,
  request body, response body, cookie or key was written to this repository, logs or this receipt.

## Import result

| Field | Result |
|---|---:|
| source records selected | 1 |
| accepted records | 1 |
| created CPAR accounts | 1 |
| rejected records | 0 |
| provider binding | native `grok_build` only |
| batch | `autoreg-oracle-build-prod-20260810-01` |
| refresh due | derived from expiry (not null) |
| priority | 101, above the known expired legacy Build entry |
| native probe | PASS |

The native probe attributed the response to the imported account and reported available health and
quota, complete canonical Chat/Responses/Messages projections, and no cross-provider fallback.

## Public CPAR matrix

Every request used the real CPAR public base URL and an existing client key. Each tuple was sent once;
there was no retry, cross-provider fallback or account substitution after a failure.

| Protocol projection | JSON | SSE |
|---|---|---|
| Responses | PASS | PASS |
| Chat Completions | PASS | PASS |
| Anthropic Messages | PASS | PASS |

Result: `attempted_calls=6`, `successful_calls=6`, with six success rows and no failure rows. JSON
responses were semantically valid; SSE streams had an HTTP success status, valid framing and a
terminal event. The receipt is value-free and marks the upstream request as sent through CPAR, not as
a direct provider probe.

The first diagnostic wrapper accidentally included historical Console cases and produced a mixed
10/12 result. It is explicitly non-authoritative. Acceptance uses only the isolated six-case matrix
receipt:

`/var/backups/cpa-rust-gateway/autoreg-oracle-build-public-matrix-only-20260810.json`

The separate two-case smoke receipt is also retained at:

`/var/backups/cpa-rust-gateway/autoreg-oracle-build-public-20260810.json`

Both remote receipts are root-only (`0600`); they contain no credential material.

## Invariants and rollback

- Active CPAR Config Version remained `p12-09-codex-official-oauth-v6`.
- CPAR listener remained loopback-only behind the existing public edge.
- The legacy CPA service, Caddy, DNS, CC Switch and grok2api were not changed.
- The imported batch can be removed with the existing value-free rollback command; the account is
  quarantined rather than falsely reported as revoked upstream.
- The production cutover backup and database preimage are under the root-only directory
  `/var/backups/cpa-rust-gateway/autoreg-cutover-20260810T071851Z`.

Verdict: `PASS_WITH_ROLLBACK_READY` for the explicit Grok Build provider-binding/import canary.
This does not certify Grok Web, Console egress, or automatic Autoreg registration scheduling.
