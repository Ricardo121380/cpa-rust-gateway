# ADR-0119: Independent controlled parallel function batches

| Field | Value |
| --- | --- |
| Status | Accepted |
| Date | 2026-10-21 |
| Task / Matrix / Contract references | #62 C2 remainder; #58 T02/T03/T08/T10; ADR-0115 through ADR-0118; [BC-PROTOCOL-013](../contracts/BC-PROTOCOL-013-controlled-parallel-functions.md) |

## Context

The real public listener/compiler/factory rejects explicit parallel=true with independent
receiver=0 and actual Attempt=0. Stock Build capability evidence lacks ParallelTools. Existing
namespace/search completion barriers do not cover a flat-only qualified parallel path. The
frozen official function-calling guide additionally specifies at most one call with false and
exactly one function under a flat forced selector. Those wire semantics can guide accurate local
software without attesting the fixed service.

## Decision

Adopt BC-PROTOCOL-013's closed scope. Use the existing private controlled assembly and actual
RouteCompiler/factory with a local compiler profile plus independent transport/adapter permission.
Keep stock capability evidence unchanged. Gate the actual Build adapter, HTTP Responses source/
target and Canonical/CanonicalBridge. Default builder/adapter rejects unqualified parallel=true;
other field qualification does not supply it. No new production activation interface is created.

Extend the existing pending completion queue and item/call identity validation to qualified
parallel decoding. Hold semantic ToolCallEnd and function OutputItemEnd through complete successful
terminal and applicable Usage validation/confirmation. Preserve provisional deltas, native output
order, multiple result call_id association and existing explicit/owned stored continuation. Apply
Responses forced named exactly-one validation without changing other source protocols' controls.
Keep provider-hosted search independent from client function batches.

## Consequences

This completes only the bounded local C2 remainder. Production BUILD_ACTIVATION_BLOCKED, actual
Codex, whole #62, #61 evidence/cost and real Provider/CI/release remain independent. No executor,
agent scheduler, second parallel framework, dependency upgrade, schema or persisted format change.
Old readers can retain existing envelopes but gain no new execution qualification. Keep prior
records, keys, original dirty content and historical failure evidence intact.

## Alternatives considered

- Add ParallelTools to the stock ledger: would falsely attest the fixed endpoint/profile.
- Enable from request/config/URL or another field: confuses independent qualification with syntax.
- Finish the first call before terminal: exposes executable work from a failed batch.
- Build a second batch/executor platform: duplicates existing state and exceeds gateway ownership.

## Validation and rollback

Retain public rejection RED and a semantic/public early-completion RED. Verify flat and cross-
namespace same-name batches, JSON and byte-fragmented interleaved SSE, reversed completion/results,
explicit and stored multi-result continuation, later dependent tool rounds and follow-up. Require
independent receiver/Attempt/Usage/ledger, negative qualifications/identities/counts/results/Usage,
no successful failed root/retry and FIN/RST release before cleanup. Regress C1-C6/ordinary serial/
Pi/Responses, bind required Full and separate Standards/Spec to immutable inputs, and run the
unchanged isolated strict actual-client regression. Restore rejection to roll back execution;
retain data and keys. No real Provider, paid account or remote mutation is authorized.
