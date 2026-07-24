# P11-01 Differential Fixture Harness execution plan

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P11-01` |
| Status | `DONE` — implementation, focused verification, and review completed; P11 Phase Gate remains pending P11-02 through P11-08. |
| Branch | `codex/p11-release-hardening` |
| Task Card | A test-only, offline corpus/classifier for CPA `v7.2.80`, grok2api `v3.0.0` / `ec6cddca7`, and Kiro-RS `c49c75e` behavior projections. It neither reads reference source nor contacts a reference service. |
| References | [P11 plan](../06-development-plan.md#17-p11---发布加固), [reference baseline](../00-reference-baseline.md#11-渠道专项参考), [channel analysis](../04-channel-reference-analysis.md) |

## Required invariant

Each committed fixture contains only an identifier, a frozen reference label, a closed vocabulary
of semantic projection markers, and an explicit classification. It cannot contain request or
response bodies, URLs, Cookies, OAuth material, API keys, database rows, or reference source.
The harness must make an omitted classification, an invalid marker, an unsupported reference, a
misclassified equality, or a `Regression` classification fail the test without echoing fixture
values.

The only accepted classifications are:

- `Compatible`: source and gateway projections are equal.
- `Intentional`: projections differ and cite a frozen plan/baseline decision.
- `Regression`: a non-approved difference; it always fails the corpus gate until fixed or
  reclassified through a reviewed decision.

## Execution sequence

1. Add a reusable test-only parser/classifier under `tests/differential/` with closed enums and
   default-deny JSON parsing.
2. Add one or more wholly synthetic, source-labelled fixtures for each frozen reference and a
   report that maps every committed fixture to its classification.
3. Test success classifications, unknown/missing input rejection, body-like field rejection, and
   the fail-closed regression path; no network client, environment credential, server, or source
   checkout may enter the code path.
4. Run focused test/Clippy/format/Secret/docs checks, independently review the fixture boundary
   and report, then commit P11-01 as `LOCAL_PASS_PENDING_PHASE_GATE`.

## Explicitly out of scope

Live reference calls, server access, reference-repository reads, fixture capture, raw
request/response retention, traffic replay, performance, fault injection, security audit,
migration/rollback drills, release packaging, P11-02 through P11-08, and any P7/P8 deferred
external authentication work.
