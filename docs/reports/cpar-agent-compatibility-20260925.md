# CPAR Agent continuation hardening — 2026-09-25

## Outcome and scope

The OMP Desktop failure was reproduced from its synthetic continuation request: the first
Responses round succeeded, but CPAR rejected the next round's top-level `reasoning` item
before dispatch. This repair covers the surrounding decode, projection, provider build,
stream finalization, saved-history replay, admission classification and request-observation
paths. It does not edit OMP or claim that every CPAR defect is eliminated.

Source evidence is the OMP local report
`/Users/huangrui/Agentprojects/only-my-pi/repo/docs/plans/2026-09-25-omp-cpar-continuation-retest.md`:
the 14:58–14:59 run completed one inference and approved `request_coding_access`, then failed
the second model round with HTTP 400; no proof-file write/read completed. The committed
`tests/fixtures/openai-responses/request-pi-continuation.json` contains only synthetic content.
No account, prompt, access token or real reasoning transcript is copied into this repository.

## Repairs

| Path | Failure prevented | Evidence |
|---|---|---|
| `gateway-core/message`, Responses decoder, router and Responses builders | Valid clear-text reasoning history rejected or flattened into visible text; completed tool-call status rejected on replay | Typed/bounded history tests; Pi fixture projection/build roundtrip; cross-protocol rejection tests |
| Generic Responses upstream decoder | Reasoning supplied only in final output-item snapshots lost; partial deltas duplicated or incomplete; late encrypted fields silently ignored | Chunked SSE no/partial/full delta tests; contradictory/ciphertext final snapshots rejected; empty reasoning alongside useful content accepted |
| HTTP stored-response continuity | Server-managed continuation silently discarded reasoning while client-managed continuation retained it | Shared public-wire encode/decode replay; stable hashed gateway item IDs; tool-call identity preserved; stored JSON/SSE three-round tests |
| HTTP ingress observation | Authenticated malformed requests absent from request statistics | Responses/Chat/Messages decode failures yield request IDs and failed terminal records, zero attempts and zero ledger entries; raw payload excluded |
| Runtime route admission | An unavailable compatible candidate was overlooked and reported as bad client input | Full matching-route classification test, including zero bindings, absent runtime and exact-model mismatch |
| Generic Responses runtime capabilities | Compiler accepted explicit stored-history support but runtime rejected the same candidate | Explicit generic `stored_responses`/`response_compaction` opt-in shape agrees with compiler; native adapters cannot inherit this exception |

## Proactive release regression

`scripts/check.sh fast` now runs `scripts/test-agent-roundtrip.py` after building the gateway.
The finite harness starts a real gateway with a private temporary database and TLS loopback
mock. It does not contact a real Provider. Failures retain private diagnostic evidence;
successful disposable state is removed.

Mandatory scenarios:

- JSON and SSE, each using both client-managed exact output replay and server-managed
  `previous_response_id`: four flows, three rounds per flow, two tool-result continuations.
- The upstream mock checks reasoning, tool status and call/result correlation on each round.
- Twelve successful external requests produce twelve attempts and twelve ledger records.
- Three invalid authenticated requests increase failed request counts without attempts or billing.
- Existing real-gateway request-chain checks additionally cover HTTP upstream failure, truncated
  streams, cancellation, durable terminal statistics, billing and two-page catalog refresh
  without expanding opened models or client permissions.

## Verification

- Local `scripts/check.sh fast`: all 41 recorded steps passed, 07:58:20–08:02:26 UTC.
  Includes workspace Rust tests, Clippy with warnings denied, Rust format, SPA/embedding checks,
  source policy, crate boundaries, management contract references, secret scanning and whitespace.
  Workspace Rust results: 1,295 passed, 0 failed, 9 ignored across 118 test executables/doc-test groups.
- New real-gateway gate: 12 successful rounds + 3 observed decode failures; attempts 12;
  successful ledger records 12. Existing request-chain: 5 requests, 2 succeeded, 2 failed,
  1 cancelled, 5 attempts; 2 successful ledger records; catalog 3 / opened 1.
- Targeted earlier checks: HTTP library 82 tests; protocol projection 29 tests; Grok Build 16
  tests. These are supporting evidence, not substitutes for the fresh full gate.
- Local logs: `output/agent-compatibility-20260925/check-fast.md`, `check-fast.log`,
  `gate-final.log`. Raw local output remains outside the commit.

## Explicit limits and rollback

- No new real-model inference was performed in this repair run. OMP's own live budget is
  exhausted; its official approval/write/read UI flow remains unverified after this change.
- Encrypted reasoning without an ownership-aware path is deliberately rejected. Clear-text
  history stays Responses-only; it is not silently converted to Chat or Messages.
- Canonical response encoding preserves clear-text semantics, not every upstream private item
  field or original upstream item ID. This is not general opaque-reasoning passthrough.
- Ingress observation covers authenticated body/decode rejection, not every later admission
  error or unauthenticated request. Unknown model metadata remains empty, not fabricated.
- No schema migration, frontend source change, permission expansion, production configuration
  rewrite, DNS/Caddy/Autoreg change or historical-data deletion is part of this patch.
- Stored-history payloads may now contain the new `Reasoning` variant. Old binaries can read
  old payloads, but cannot replay newly stored reasoning-bearing payloads. A binary rollback
  preserves data and ordinary service, but those new continuations require the repaired binary
  or a new conversation. Production-copy startup/rollback checks do not prove new-payload
  backward decoding. Prefer roll-forward for this case; do not erase history to conceal it.

## Release status

Production runs `f4ebf4d75c111f8e0fa8e4f9d342a01bfb8da723` as of 08:43:53 UTC.
Implementation commit: `7468d6d`; safe failure-diagnostic follow-up: `f4ebf4d`.
See [production evidence](cpar-agent-production-20260925.md), including the unreproduced first-CI
503 and the distinction between isolated mock acceptance and real OMP acceptance.
