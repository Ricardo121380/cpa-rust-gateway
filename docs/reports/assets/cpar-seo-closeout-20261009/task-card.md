# Task card: remaining specification and acceptance evidence closeout

Initial status: IN_PROGRESS. Recorded after baseline/rule reading and before implementation exploration.
Task level: L (public protocol admission, authorization recovery, isolation and real embedded UI).
Fixed base: `27ee36174f54bac059694a0870e452f9ac632ce0`.
Branch: `codex/prism-v4-delivery`; actual initial HEAD matches the fixed base.
Finding IDs: S-01, E-01, E-02, E-03, E-04, O-01, E-05 and affected A-02–A-08 integration.
These IDs are independent of the original 13 locally resolved P2 findings.

## Scope, working budgets and completion conditions

| Group | Planning budget | Required result |
|---|---:|---|
| Baseline, rules, preservation, evidence inventory and E-05 decision entry | 30 min | actual source/dirty baseline, per-item gaps; one explicit user choice card |
| S-01 ADR structure and compatible rollback | 20 min | required ADR sections, unchanged semantic rules, ADR-0103/key-version compatibility; static/docs checks |
| E-01–E-03 presets and committed device recovery | 90 min | distinguish all four real presets; committed-then-lost recovery and exchange count; production-compatible Kiro fixture classification |
| E-04 public isolation, O-01 phase-only, approved E-05 | 90 min | public positive/negative matrix with exact execution identity and counts; independent O-01 RED/GREEN if confirmed; only explicitly approved E-05 rule |
| Actual gateway `/admin-ui/` integration | 90 min | isolated real serve/SQLite/management handler and embedded page, EgoLite operations and backend receipts; explicit uncovered fault branches |
| Final clean-source Full, frontend gates and independent reviews | 60 min | one batch Full, required frontend gates, Standards/Spec complete fixed-base diff coverage and correction checks |
| Batch report, evidence index and local #57 draft | 30 min | per-item status/source/receipts, exact limitations and next batch; path-limited local commits |

Budgets are estimates, never permission to omit work. Record real timestamps, overruns,
failures and blockers. Main agent owns all changes, decisions, final execution and commits.
Generic scouts are read-only, one-turn, `fork_turns=none`; independent exploration/review
only. After dispatch the main agent waits and converges; foundational documents remain
main-agent reads. At most one implementation item is IN_PROGRESS at a time.

## Authorization and rules

User authorizes necessary local backend/frontend (`web/prism`), tests, fixtures, docs,
temporary loopback gateway/upstreams, EgoLite, reviews, evidence and local commits.
Preserve user AGENTS.md and unrelated tracked/untracked material. Use exact paths in
staging. Crossings require same-commit cross-boundary log and trailer; authoritative
management contracts synchronize one way through sync-contract.

No real Provider/account or paid inference; no production credentials, data, keys or
services; no deployment, publication, push, GitHub writes/closure or unspecified external
model. GitHub reads use `docs/agents/issue-tracker.md`. Keep #57/release, real channels,
remote CI, new signed artifacts, deployment and production separate.

## Initial acceptance ledger

| Item | Current fact / reusable evidence | Actual gap | Planned action / acceptance | Initial status |
|---|---|---|---|---|
| S-01 | ADR-0100 accepted semantics; ADR-0103 documents new Usage identity | mandatory structure and explicit safe rollback | structure only, references/content/diff/docs gate | PENDING |
| E-01 | A-02–A-04 save/application receipts at a98bfd5e | previous acceptance did not distinguish four presets | real definitions, exact submitted mapping/connection/runtime, positive/negative cases | PENDING |
| E-02 | A-05 original session/task recovery | old unknown-before-save fixture does not prove commit-then-loss | controlled upstream, committed fact before loss, original receipt, same account and one exchange | PENDING |
| E-03 | A-06 material-aware UI; original fixture finding | Kiro fixture classified bearer/null | compare production contract, correct fixture, state/action/recovery tests | PENDING |
| E-04 | previous public HTTP 410 checks | exact same-account/different-connection, model and shared-Upstream Chat cells need mapping | reuse only matching cells; public HTTP execution/receive-count matrix | PENDING |
| O-01 | independent historical phase-only JSON observation | current JSON/SSE reproduction and negative matrix absent | official current fields; reproduce, minimal fix if needed, preserve phase through store/history | PENDING |
| E-05 | #36 rejects revision changes; BC-RESP-003 strictly proves Build exception | authoritative rule conflict | choice card sent; preserve behavior pending explicit decision | AWAITING_USER_DECISION |
| `/admin-ui/` | previous EgoLite synthetic 16 scenes | actual management-listener integration not executed | isolated actual serve and embedded UI; affected E/A flows and handler/store facts | PENDING |

## Evidence requirements

Each item records current fact, reusable evidence, gap, actual action, acceptance and
final state/source. Distinguish documentation gaps, missing execution evidence, confirmed
bugs, observations and user decisions. A historical static finding is not a new RED.
Keep setup/filter/fixture failures separate. Receipts record full command, UTC timestamps,
exit/counts, exact source/clean state and named SHA256 objects (raw log bytes, archive or JSON).
Final runtime validation uses a clean isolated committed checkout. Final doc-only delta
uses the docs gate; do not relabel old runs. E-05 silence never authorizes either choice.

## Rules read

User attachment (2026-10-09), applicable AGENTS.md (including preserved local modification),
cross-boundary log tail, issue-tracker/domain rules, CONTEXT.md, ADR index, ADR-0100/0103,
BC-RESP-003, development plan execution/quality gates, frontend development plan and prior
eight-P2 report/supplement, release review findings and O-01 observation.

## Execution settlement

Status: IMPLEMENTATION_AND_EXECUTION_COMPLETE; report/review closeout in progress.
E-05 remains AWAITING_USER_DECISION, with no user choice or approved rule.
Actual stock serve device commit-loss UI remains NOT_RUN (safe injection unavailable).
Final runtime/acceptance source: `5701e0e5f5fe7e6b663561b8a0fcce02885e2bb1`.
Clean isolated checkout: `output/cpar-seo-closeout-20261009/accepted-source`.

- Final Full: 45/45 PASS, Rust 1493 passed / 0 failed / 12 inherited ignored, 244.271 s.
- Prior clean Full attempts: `0968036f` failed Clippy doc-markdown (76.143 s);
  `b0710eb4` passed 45/45 (311.098 s), followed by an actual UI origin oracle failure.
  Both are retained with their original source and result.
- Frontend: type-check PASS, 494 tests / 68 files PASS, check:full including double build in Full.
- Actual embedded UI: 18 PASS, exact serve PID 56948, four controlled single-call receives.
- Fixture: 17 PASS at b0710eb4; all frontend/fixture-script blobs equal final 5701e0e5.
- Public HTTP: 527 PASS = reused 410 + 117 new checks on 14 rows; targeted 339
  included 222 common setup checks and must not be double-counted.
- Preservation: initial 8199 objects = 3804 tracked + 4395 untracked; only authorized
  existing paths changed. User AGENTS.md and all original untracked hashes remain equal.
- F-01 actual HTTPS reuse is a newly confirmed local defect, fixed independently of the 13 P2.

Original budgets remain historical estimates. Full retries and browser/setup/oracle failures
are reported separately; parallel command durations are not added as wall-clock task time.
No remote write, push, deploy, real Provider/account or paid call was performed.
