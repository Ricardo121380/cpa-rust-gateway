# #74 quota history local delivery

## Scope and implementation

- Issue: [#74](https://github.com/Ricardo121380/cpa-rust-gateway/issues/74), OPT-07, parent #58.
- Worktree: `worktrees/cpar-74-quota-history-20261025`; branch `codex/74-quota-history`.
- Common baseline: `444558a8de930f5f1febd5a40ec5bdbbc30a15fb`.
- Implementation: `c34834b0e37f8fef59fc62f85ea8bc40fd33274f`.
- Review fixes: `c4bdd7e181e6d698106d728ba8dccb753e74c827`.
- Final code / local envelope fixes: `407a962980681d31dcbbbe5fdce448aed576c8d4`.

Account quota observations now append to an independent SQLite history. Existing ordinary
Codex/Claude/Kiro reads, native Build/Console/Web reads and Kimi management/background metadata
supply samples; cache hits are not new samples. The change adds no scheduler or polling frequency.
Passive router 429/header state and the unwired native Quota worker are outside this history path.

A transactional fence checks the original account revision, material and lifecycle before append.
Ordinary runtime readers and caches use the current leased material revision. Same-time identical
samples coalesce, conflicting samples reject the batch, and older same-revision evidence cannot
replace later observations. Account deletion retains prior history. History has no serving authority.

The [contract](../contracts/BC-MGMT-018-account-quota-history.md) and
[ADR](../adr/ADR-0129-account-quota-history.md) define observation, confidence, completeness,
freshness and gap semantics. `projected` covers recognized fields, never whole-account completeness.
Missing quantities remain null. Changed reported reset times are provider evidence, not a witnessed
reset, recharge, actual consumption or billing event. Token Usage and cost retain their existing owners.

## Acceptance evidence

All seven feature criteria pass at the bounded local evidence levels below. The required Full gate
is **FAIL / blocked by an inherited supply-chain advisory**; this is not an all-gates-green delivery.

| Issue acceptance criterion (local PASS) | Local implementation and evidence |
|---|---|
| Reuse existing observations, retain account/provider/window/source/time/confidence/completeness | Independent store, sanitized projections and existing runtime/management readers; HTTP/projection tests |
| Separate unknown, failed, stale, partial and exhausted | Independent status/completeness/freshness fields; explicit nulls; HTTP and controlled process assertions |
| Sampling, duplicate rules and late-response safety | Atomic fence; duplicate/conflict/older tests; HTTP mutation tests; real process disable/delete race |
| Bounded history query and time/account/window filters; gaps/reset evidence | Required `[from_ms,to_ms)` of at most 31 days, page limit 1–200, append-ID cursor; HTTP filters/gap/reset checks |
| No inference probe, automatic enable/recovery or new real polling | Existing readers only; process has background tasks disabled and an outbound OS fence |
| Management HTTP, cross-account/window, expiry/failure, concurrent mutation and restart | Rust management HTTP tests plus stock process acceptance using one controlled TLS billing peer, two accounts and an actual exit/new process |
| Keep quota, Usage and cost distinct; no secrets in history | Allowlisted numeric/structural evidence; process checks zero event/ledger rows and absence of synthetic credential/identity text |

## Validation and review

- Required Full gate: **FAIL**, with 42 steps passing and dependency policy failing on
  `RUSTSEC-2026-0387`. The script stopped before its final RustSec audit step. The separate
  `cargo audit` also failed on the same advisory. See [gate receipt](issue-74-quota-history-evidence-20261025/full-check.md)
  and [supply-chain evidence](issue-74-quota-history-evidence-20261025/supply-chain.txt).
- Rust workspace tests: **1675 passed, 0 failed, 21 ignored**, aggregated from 132 test summaries.
  All four new quota history tests passed. Format, Clippy, stock serve, controlled multi-turn
  regression, source policy, crate boundaries, 350 contract references and secret scanning passed.
- Final controlled process run 3: **17/17 checks passed**. Two processes (53296 and 53555) exited
  with code 0. The receiver saw exactly six `/v1/billing?format=credits` reads. Restart/history query
  added no upstream call; history readback matched and event/ledger row counts stayed zero.
  See the [sanitized process receipt](issue-74-quota-history-evidence-20261025/process-receipt.json).
- Process code/harness were clean at `407a962980681d31dcbbbe5fdce448aed576c8d4`. The receipt's
  nonempty diff hash is a documentation-only report draft, preserved locally as
  `output/issue-74/process-source-docs.patch`; production code and acceptance scripts had no diff.
  Final follow-on changes are documentation and evidence only.
- Standards review of the implementation: zero actionable findings.
- Spec review initially found three issues: native empty config filtering, missing history wiring
  silently skipping recording, and loss of Kimi partial-window evidence. All were fixed, each has
  regression coverage, and a separate read-only review confirmed zero remaining actionable findings.
- Review scope uses the fixed common baseline. Kimi partial/source coverage is a projection test;
  it is not evidence of a real Kimi HTTP request or scheduled real-provider run.

Failures remain in local evidence: the first Full stopped at the new error enum's
`needless_pass_by_value` warning; deriving `Copy` fixed it without changing the gate. The first
process run stopped at database readiness because the filesystem fence denied ancestor metadata;
the envelope now permits only ancestor metadata, retaining private file and outbound restrictions.
Initial compile/targeted-test iteration logs are retained under `output/issue-74` as well.

## Remaining gate blocker

`RUSTSEC-2026-0387` (published 2026-10-23) affects the locked `crypto-bigint 0.5.5` through
`provider-grok -> p256 0.13.2 -> elliptic-curve 0.13.8`. The baseline and code HEAD have identical
Git blobs for Cargo.lock, workspace Cargo.toml, provider-grok/Cargo.toml and deny.toml. The only
manifest change moves the already existing `time` dependency from development to normal use.
The independent baseline-lock audit reproduces the same advisory against the current advisory DB.
This establishes inherited dependency exposure, not a historical baseline Full result.

The [official advisory](https://rustsec.org/advisories/RUSTSEC-2026-0387.html) reports timing
variability in operations intended to be constant-time and a fixed range of `>=0.7.6`. The current
`elliptic-curve` constraint is `crypto-bigint ^0.5`, so a normal lock-only update cannot reach the
fix. A broader cryptographic dependency migration is outside quota-history scope. No ignore,
exception, dependency upgrade or downgrade was introduced. Exploitability in this application has
not been assessed. This blocker must be resolved in separately authorized dependency work before
claiming Full success; there are no unresolved #74 feature-review findings.

## Shared interfaces and migration

- New management route: `GET /admin/quota-history`, existing auth/hidden-404 policy and `no-store`.
  Required `from_ms`, `to_ms`, `limit`; optional `after_id`, `account_kind`, `config_version_id`,
  `account_id`, `provider`, `window`. Native config scope is the empty string. Invalid input returns
  400; unavailable persistence/query returns 503. This endpoint makes no upstream request.
- Pagination is live append-ID pagination, not a frozen #72 export. Keep filters fixed while paging.
  A window filter also includes windowless failure/unknown evidence. No automatic retention or
  pre-upgrade history reconstruction is provided.
- Management state gains history-store injection; deployment opens it on the existing control DB.
  Existing native usage now requires successful history wiring and documents its 503 boundary.
- Additive Kimi observation field: `quota_partial`. Runtime quota caches become revision-bound.
- Migration `0039_quota_history` advances schema 38 to 39, adding one strict table and time/series
  indexes. No existing Usage/ledger table is altered. No account foreign key means account deletion
  does not erase history. The down migration deletes this history table; backup is required before
  any explicitly authorized downgrade that would discard it.
- Shared integration surfaces: management route registry and OpenAPI, deployment/runtime injection,
  migration registry/schema number, ADR-0128, BC-MGMT-018, traceability and engineering roadmap.
  Concurrent #68/#72 numbering and code integration must be reconciled later; no sibling changes
  were imported or rewritten in this branch.

## Evidence boundaries

All credentials and TLS material used by the process harness are generated synthetic values in its
private output directory. Database, keys, captures and process logs stay under ignored local output;
only sanitized validation receipts may be committed. The macOS envelope uses dynamic ports,
child-only DNS/connect interposition, a single allowed loopback peer and background tasks disabled.

Real Provider and inference: NOT_RUN. Provider cost: UNKNOWN. Actual clients, fixed shared service,
remote CI, release, deployment, push, merge and issue closure: NOT_RUN. Local passing checks do not
establish any of those qualifications. No real credential files or sibling worktrees were modified.
