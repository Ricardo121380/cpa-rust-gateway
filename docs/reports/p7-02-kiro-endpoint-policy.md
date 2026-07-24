# P7-02 Kiro IDE/CLI endpoint policy report

| Field | Value |
|---|---|
| Task | `P7-02` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Scope | Strict API Region, fixed IDE/CLI URL, non-secret Header/Origin policy, API-key marker, and Thinking placement |
| References | `C39-C40`; [ADR-0047](../adr/ADR-0047-kiro-endpoint-policy.md); [BC-PROVIDER-005](../contracts/BC-PROVIDER-005-kiro-endpoint-policy.md) |

`KiroEndpointPolicy` is a pure local mapping from Endpoint kind and API Region to an exact URL and
deterministic non-secret request head. It rejects hostname/path injection through Region input. The
credential family controls only the `tokentype: API_KEY` marker; no token or API-key value is copied
into the policy.

P7-02 deliberately distinguishes the API Region used for host derivation from P7-01 Enterprise
`auth_region` used for refresh. It records future Thinking placement but does not map a Canonical
request. `profileArn`, machine/client identity, outbound authorization, transport, and real endpoint
validation remain deferred to their assigned P7 Tasks.

## Verification and review

| Check | Result |
|---|---|
| `cargo test --locked -p provider-kiro --test p7_02_endpoint_policy` | PASS; exact IDE/CLI URL/Header/Origin/Thinking snapshots, API-key-only marker, and bounded hostname-safe Region regressions |
| `cargo clippy --locked -p provider-kiro --all-targets --all-features -- -D warnings` | PASS |
| `ruby scripts/check-crate-boundaries.rb` | PASS; `url` is limited to fixed local URL validation |
| `./scripts/check.sh docs` | PASS; links, single-task plan state, secret scan, and whitespace |

Review confirmed that API Region accepts safe multi-segment AWS values such as `us-gov-west-1`,
while it still rejects hostname/path injection. The policy has no caller-controlled URL or network
operation, and it cannot contain credential secret values.
