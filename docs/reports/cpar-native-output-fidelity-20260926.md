# Native Responses fidelity — 2026-09-26

Status: implemented and verified locally; not released. Production remains `4d5ce334`.
Base: `d99bb4f6ba7f018f447e6b3f4356415afab3b59a`. This report covers native Grok
Build output fidelity, not arbitrary opaque reasoning or all Provider implementations.

## Implementation

- Optional `OutputItemStart` / `OutputItemEnd` preserve native output identity/order in the
  canonical stream. Core validates lifecycle and bounded ASCII identities; Debug stays redacted.
- The native Build decoder retains summary-only reasoning, multiple summary/content parts,
  empty items, original message/tool/reasoning IDs, and interleaved completion order. Partial
  text is completed once. Contradictory final snapshots and changed output order fail closed.
- The public Responses JSON/SSE codec renders native metadata rather than generating replacement
  IDs or folding summary into body. Explicitly supported native metadata remains Responses-only;
  plain text/tool cross-protocol projection preserves established Chat/Messages behavior.
- Stored-history reconstruction preserves native IDs; only gateway-generated lookup-bearing IDs
  are hashed. This is a helper-level test, not permission to enable native stored-response capability.
- Blank and empty-object tool arguments still normalize to `{}` under BC-PROVIDER-003/ADR-0044.
  A new streamed-empty-object regression exposed a downstream comparison mismatch; the encoder
  now accepts only this defined normalization exception. Other argument inconsistencies fail.
- Non-null encrypted reasoning is rejected even if it appears only in a terminal snapshot.
  No unowned opaque forwarding, continuity-store activation, capability expansion, new retry,
  schema migration, management API or frontend change is introduced.

Design: [CR-RESPONSES-NATIVE-FIDELITY-001](../change-requests/CR-RESPONSES-NATIVE-FIDELITY-001.md).

## Verification and evidence separation

`crates/provider-grok/tests/native_output_fidelity.rs` exercises the real native decoder →
runtime projector → public JSON/SSE encoder → client replay. Cases cover summary/body separation,
original IDs, partial/empty/multiple parts, invalid terminal fields, empty-object normalization,
reverse-order item completion and encrypted-field rejection. Existing native adapter acceptance
also asserts the original fixture IDs across eight JSON/SSE/legacy combinations.

The first full local `scripts/check.sh fast` passed 41 steps, including 1,304 Rust tests,
9 pre-existing ignored tests, strict Clippy, format, management SPA, contract and source boundaries.
It predates the final interleaved and streamed-empty-object additions. It is supporting evidence,
not the final revision's result. The fresh final gate passed all 41 steps with **1,305 Rust tests passed, 0 failed,
9 pre-existing ignored**, including both final additions. See [final check receipt](evidence/cpar-stability-20260926/local-fast.md)
and [real-gateway summaries](evidence/cpar-stability-20260926/local-gateway-summary.json).
The gate's strict Clippy failures during development (explicit imports, bounded helper size,
redacted Debug counters and exhaustive test matches) were corrected, not waived.
The streamed-empty-object negative result was reproduced before the final comparison fix.
Raw local log: `/tmp/cpar-native-fidelity-final.log`.

The real gateway + TLS loopback mock gate passed four three-round flows: 12 successful requests,
12 attempts and 12 materialized ledger entries; three malformed authenticated requests failed
without attempts/billing. Its separate request-chain check passed success, upstream failure,
truncation, cancellation and paginated catalog refresh without widening grants. This HTTP gate
uses the generic compatible adapter; native Build coverage is the separate Rust adapter matrix.
Neither is presented as a new real Provider call.

## Unexplained failures

The old first-CI `503` occurred on the third nonstreaming client-managed-history round at
`7468d6d`. Its assertion retained only `(False, 2, 503)`; no matching error/runtime diagnostic was
captured. The existing later harness now retains error code and runtime availability on failure.
No repeat of the old failure occurred in this round's local gate. Its historical cause remains
unknown; passing reruns do not establish a fix.

OMP's first writer Connection error is a separate observation: its window had no matching CPAR
request, but that absence and a later successful health check do not establish cause or no charge.
The writer retry's recursive cancellation crash belongs to its acceptance script; the latest OMP
report distinguishes it from CPAR behavior. This patch does not edit that project or its budgets.

## Existing real-client acceptance cross-check

The latest OMP source report is
`/Users/huangrui/Agentprojects/only-my-pi/repo/docs/plans/2026-09-26-omp-cpar-real-acceptance.md`.
It records a four-round approval → write → read → Host succeeded run on deployed `4d5ce334`.
On 2026-09-26 01:30 Asia/Shanghai we independently read the four exact CPAR request IDs and
confirmed four request/attempt/usage/terminal sets and four `unpriced` ledger rows. All terminals
are succeeded; amounts remain null. Token totals agree with the OMP report (15,774 including
cached input once). See [sanitized production readback](evidence/cpar-stability-20260926/omp-production-readback.json).

This closes the previously missing CPAR request/ledger evidence for that existing run. It does
not validate the new local fidelity patch in production, real OAuth, all channels, writer terminal
success, or cancellation of an already-running shell. No new Provider inference occurred here.
The CPAR diagnostic budget remains 2/2; OMP's latest ledger records tasks 3/3 and retries 5/5.

## Rollback / next gates

Old stored payloads remain readable by the new binary. New optional item lifecycle events require
an aware binary for replay; switching to an older binary must retain data and may require a new
conversation. No backward replay of these new payloads is claimed.

Before release: exact-revision formal delivery/signing, artifact test and current
production-copy rollback checks. After release: any new live-model test needs its own remaining
or newly authorized bounded allowance; do not reset either existing budget.
