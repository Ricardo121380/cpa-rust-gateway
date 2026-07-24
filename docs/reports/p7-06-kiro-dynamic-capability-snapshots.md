# P7-06 Kiro dynamic capability snapshots report

`dynamic_catalog` establishes the per-Credential Kiro model and subscription snapshot boundary.
It parses bounded paired source observations, retains a complete immutable last success with
P4-02 Fresh/Stale/Expired deadlines, and aggregates only current or eligible-stale results. One
credential's failed probe is content-free and cannot clear or block another credential's model
contribution.

The tests cover safe subscription projection/redaction, malformed and duplicate source rows,
exact freshness/refresh/expiry boundaries, current-plus-stale aggregation, missing and expired
Credential isolation, conflicting model limits, all-failure safe output, duplicate input IDs, and
non-monotonic replacement rejection. The union deduplicates real source IDs and never generates
a second `-thinking` model.

No Kiro HTTP request was sent. OAuth/API-key injection, endpoint behavior, failure
classification, Tool/Thinking semantics, and real E2E remain later P7 work.

## Verification and review

| Check | Result |
|---|---|
| `cargo test --locked -p provider-kiro --test p7_06_dynamic_catalog` | PASS; eight parsing, freshness, stale/expired, union, redaction, duplicate, and failure-isolation regressions |
| `cargo clippy --locked -p provider-kiro --all-targets -- -D warnings` | PASS |
| `ruby scripts/check-crate-boundaries.rb` | PASS after explicitly constraining the new read-only `gateway-catalog` timing-policy edge |
| `./scripts/check.sh full` | PASS; workspace format, Clippy/tests, source/secret policy, crate boundary, links, dependency policy, and RustSec audit |

Review focus: per-Credential state remains atomic, a bad source record cannot replace a good
snapshot, unknown subscription data cannot claim paid access, and a union never creates a virtual
Thinking model. This task is `LOCAL_PASS_PENDING_PHASE_GATE`.
