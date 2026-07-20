# ADR-0020 Authorized real-test Endpoint validation

| Field | Value |
|---|---|
| Status | Proposed |
| Date | `2026-07-20` |
| Task / Matrix / Contract references | `P3-10`; `C16`, `G05`, `G12-G15`, `G21`, `K03-K06`, `L20-L31`; [BC-E2E-002](../contracts/BC-E2E-002-authorized-real-test-endpoint-validation.md) |

## Context

P3-09 proves the P3 aggregation slice against two controlled loopback HTTP peers. Its JSON and
SSE decoder is deliberately fixture-limited and test-only; it is not a production claim about any
deployed OpenAI-compatible relay. P3-10 must now establish a narrow, reproducible compatibility
check against two separately authorized test relays without turning an ad-hoc probe into a service
deployment, a generic production decoder, or a source of committed secrets and raw provider data.

The repository has no tracked real-test Endpoint, model, credential, proxy profile, or approved
request budget. An operator-controlled ignored local configuration may hold those values only for
an explicitly authorized run. Ambient `OPENAI_API_KEY`, `CODEX_API_KEY`, shell history, and
previously supplied chat text are not P3-10 configuration and must never be selected implicitly.

## Decision

1. P3-10 adds a dedicated manual validation harness under an ignored-test target. It is compiled by
   ordinary local/CI checks but never contacts a network unless its explicit live-test switch and
   all endpoint-specific variables are supplied by the operator.
2. The harness will use only explicit `P3_10_*` environment variables sourced from an ignored local
   file or another operator-controlled secret mechanism. It will not read `.env` automatically,
   inspect ambient API-key variables, write credentials to SQLite, or persist them in a tracked
   file.
3. Each target is tested through the existing P3 request builder, egress admission, client pool,
   scheduler/orchestrator, Snapshot-authenticated Actix boundary, and event sink. The target is a
   test-only composition path, not an application listener or a new production Provider adapter.
4. The harness accepts exactly four live calls: one minimal non-streaming and one minimal SSE request
   per target. Every request has the fixed non-sensitive input and `max_output_tokens=32`. Its
   two-Candidate Route has `max_attempts=1`, so a failed Candidate cannot fail over or spend an
   unapproved retry. It performs no account mutations, model discovery, quota probes, or bulk traffic.
5. Tracked evidence records only opaque target labels, protocol mode, status/content-type category,
   bounded latency bucket, event-shape result, and boolean correlation/redaction checks. It never
   records a URL, Authorization header, credential, Client Key, request body, response body, SSE
   frame, provider response ID, upstream model, or raw trace. If raw troubleshooting material is
   necessary, it remains in the ignored `docs/reports/private/` area and is discarded after review.
6. A protocol mismatch, authorization failure, unexpected billing/consent condition, or any output
   that cannot be safely summarized stops the run. It becomes a redacted compatibility finding; it
   does not authorize broad decoder changes or automatic fallbacks.
7. Under `CR-P3-G3-001`, the P3-10 public model is the test-only
   `p3-chatgpt-compat` alias. It is deliberately distinct from a provider model identity: each
   Candidate continues to receive its explicit private upstream-model mapping, while every
   client-visible response is checked for the public alias. This change does not select or discover
   an upstream model and does not revise P3-09's loopback-only fixture naming.

## Consequences

The live check is opt-in, bounded, reproducible, and does not make CI or ordinary developer tests
depend on external accounts. P3-10 can distinguish a real relay compatibility result from P3-09's
synthetic proof while preserving the crate boundaries and secret rules established by P2/P3.

The implemented target shares P3-09's test-only composition harness, so a later real-target result
uses the same Snapshot, Client Key, egress, transport, attempt, decoder, event, and public-model
boundaries that the deterministic Mock E2E already exercises. `direct` and explicit local-DNS
SOCKS5 profiles are isolated by the existing P3-02 transport; optional private-network CIDRs remain
per-target P2 egress allowlists rather than a TUN or system-proxy mutation.

P3-10 does not make the application deployable, validate a production service Base URL, introduce a
generic OpenAI Responses decoder, persist events, or replace P4/P5 work. The ADR becomes
`Accepted` only after the implementation and its local review show that the live-test guard and
redaction behavior meet this decision.

## Alternatives considered

- Reuse P3-09's loopback fixture as a claim about deployed relays: rejected because it cannot
  establish real protocol compatibility.
- Put a relay URL/key in an ordinary integration test: rejected because CI, logs, and source history
  would become an unsafe secret and billing boundary.
- Add a gateway server/management deployment before validation: rejected because deployment belongs
  to later planned work and would expand P3-10 beyond its validation scope.

## Validation and rollback

Before any external request, review the ignored-test guard, the fixed request cap, and the absence
of secret-bearing tracked output. Rollback is local: remove the ignored test target and its
documentation or unset its `P3_10_*` variables. No deployed configuration, database record, or
external account state is changed by preparing this path.
