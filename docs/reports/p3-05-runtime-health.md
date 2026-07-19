# P3-05 Sharded runtime health report

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P3-05` |
| Matrix / behavior | `E08`, `E11`, `E12`, `K06`, `L30`; Behavior 2/3/17/20 |
| Date | `2026-07-20` |
| Branch | `codex/p3-05-runtime-health` |
| Rust | `1.97.1` |
| Result | PASS locally after review; GitHub Fast/Full acceptance pending |

## Delivered scope

- Added `gateway-router::RuntimeHealthRegistry`: a process-local, non-persistent registry keyed by
  either an Endpoint or an Endpoint/Credential pair. The default has `64` independently locked
  shards; each retains at most `1024` state entries and an availability lookup reads exactly one
  deterministic shard.
- Added injectable system/test clocks, safe bounded-state errors, and the narrow
  `CoolingDown { until_ms }` / `CircuitOpen { retry_after_ms }` state model. Cooldowns resume at
  their deadline; Circuits require an explicit `mark_healthy` recovery even after `retry_after_ms`.
- Added health-aware `RouteCredentialScheduler` selection. It filters an unavailable Endpoint
  before reading its pool, then applies Endpoint/Credential availability to each pool slot before
  CAS lease reservation, preserving a healthy sibling Credential and existing Candidate weights.
- Added `EndpointCredentialPool::try_lease_eligible`, which exposes only a stable non-secret
  Credential ID to the predicate and cannot reserve capacity for a rejected slot.
- Added [ADR-0015](../adr/ADR-0015-sharded-runtime-health.md) and
  [BC-HEALTH-001](../contracts/BC-HEALTH-001-sharded-runtime-health.md), with synthetic tests for
  isolation, expiration, recovery, monotonic deadlines, capacity reclamation, lease safety, and
  fail-closed scheduling.

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-router -p gateway-upstream` | PASS; 25 Router and 27 Upstream tests, including state isolation, recovery, pool predicate lease safety, and health-aware two-stage selection |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked -p gateway-router -p gateway-upstream --all-targets --all-features -- -D warnings` | PASS |
| `ruby scripts/check-crate-boundaries.rb`, `ruby scripts/check-source-policy.rb`, and `ruby scripts/check-doc-links.rb` | PASS; 21 package boundaries, 53 Rust files, and 88 Markdown documents |
| `git diff --check` | PASS |
| `scripts/secret-scan.sh --staged` and `git diff --cached --check` | PASS; all P3-05 files were included in the staged Secret and whitespace checks |
| `./scripts/check.sh fast` | PASS; complete workspace Fast gate |
| `./scripts/check.sh full` | PASS; complete workspace Full gate, including dependency policy and RustSec audit; existing duplicate-version notices are policy-allowed warnings |

## Review

Review passed. The registry is bounded and keeps contention local: a request-time lookup performs
one clock read and one read lock on the key's fixed shard; a mutation writes that same shard only.
Full shards reclaim only expired Cooldowns on a new insertion; live Cooldowns and open Circuits are
never evicted. No request-time operation reaches SQLite, a configuration file, a network client,
or a global scheduler lock.

The review verified strict state boundaries and recovery semantics: Endpoint state is
protocol-specific, Endpoint/Credential state includes both identities, a shorter deadline cannot
weaken an existing block, Cooldowns become eligible at their deadline, and Circuit state remains
closed until explicit recovery. It also verified that temporary runtime state does not mutate
`RouteSnapshot` or model visibility.

The review added three targeted regression assertions before acceptance: a shared Credential ID is
unaffected at a different Endpoint; an unavailable runtime clock yields the existing safe
`CredentialUnavailable/Credential` selection error before any pool lease; and a rejected pool
predicate leaves its Credential's lease count at zero. P3-05 remains limited to state and
eligibility: it adds no failure classification, Attempt, retry/failover, HTTP, probe, EWMA,
persistence, event, or Provider behavior.

## Scope and deferred work

P3-05 does not classify HTTP/Provider/transport failures, change durable Credential status, persist
health state, send active probes, use EWMA, implement half-open Circuit recovery, create an Attempt,
retry or fail over, build/send HTTP, parse a response, emit events, publish `/v1/models`, or contact
a deployed Endpoint. P3-06 owns Attempt classification/exclusion/retry and first-semantic-event
behavior; P4 owns active probes, EWMA, model state, and controlled Circuit recovery. All fixtures
use synthetic IDs and no deployed Credential or Secret.

## GitHub CI

The implementation commit's GitHub Fast and Full gates must both pass before the P3-05 acceptance
record is finalized. Its separate verification-record and final status commits must also pass the
same workflow before P3-06 can begin.
