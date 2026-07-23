# P7-08 Kiro failure-owner classification report

`failure_classification` makes Kiro failure attribution explicit and non-destructive. It accepts
only a network phase or an HTTP status with a bounded safe signal, returns the Canonical error and
the sole permitted remediation owner, and retains no upstream values. Unknown `403` remains an
egress event, while account forbiddance needs independent evidence. Quota, account 429, ordinary
429, model availability, and post-semantic stream interruption remain distinct.

No Kiro HTTP call, Credential/account state mutation, route change, retry, or probe occurred.
P7-09 alone will attach the classification to a bounded real adapter and perform clean-room
differential evidence.

## Verification and review

| Check | Result |
|---|---|
| `cargo test --locked -p provider-kiro --test p7_08_failure_classification` | PASS; network phases, 401, unknown/confirmed 403, model, quota, account/ordinary 429, transient, precedence, and no-value diagnostics. |
| `cargo clippy --locked -p provider-kiro --all-targets -- -D warnings` | PASS. |
| `ruby scripts/check-crate-boundaries.rb` | PASS. |

Review focus: no generic HTTP status may silently poison an account, and no classification action
is itself a state mutation. This task is `LOCAL_PASS_PENDING_PHASE_GATE`.
