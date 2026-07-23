# P11-01 clean-room differential Fixture Harness report

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P11-01` |
| Status | `DONE` — P11-01 local implementation, verification, and review are complete; the P11 Phase Gate remains pending later P11 tasks. |
| Branch | `codex/p11-release-hardening` |
| Corpus location | [`tests/differential/fixtures/`](../../tests/differential/fixtures/) |
| Harness location | [`tests/differential/harness.rs`](../../tests/differential/harness.rs) and [`p11_01_differential_fixtures.rs`](../../crates/gateway-core/tests/p11_01_differential_fixtures.rs) |
| Frozen sources | CPA `v7.2.80`; grok2api `v3.0.0` / `ec6cddca7`; Kiro-RS `c49c75e` |

## Scope and safety boundary

This is an offline, test-only clean-room corpus.
Its JSON fixtures hold only a source label, semantic subject, closed projection markers, classification, and a frozen decision for intentional differences.
Committed files contain no request or response bodies, URLs, Cookies, headers, OAuth values, tokens, API keys, database rows, account identities, source checkouts, or captured server material.

The harness has no HTTP client, filesystem traversal, environment lookup, credential type, reference-source reader, or server path.
`include_str!` embeds the six reviewed synthetic files at compile time.
Default-deny parsing, fixed source/subject and subject/marker pairs, an 8 KiB input cap, duplicate-marker rejection, and forbidden body-bearing field-name rejection require explicit review of new corpus material.

## Classification rule

| Classification | Acceptance rule |
|---|---|
| `Compatible` | Reference and gateway projections must be exactly equal; a decision is forbidden. |
| `Intentional` | Projections must differ and cite one frozen design decision. |
| `Regression` | Always fails the fixture gate. It must be fixed or receive a reviewed intentional classification. |

An omitted classification, unknown field/marker/reference, invalid source/subject or subject/marker pairing, duplicate marker, oversized input, forbidden field, equality marked as intentional, or difference marked as compatible fails closed.
Errors expose only a stable category, never fixture values.

## Committed differential corpus

| Fixture | Reference | Semantic subject | Classification | Decision / outcome |
|---|---|---|---|---|
| `cpa-canonical-lifecycle` | CPA `v7.2.80` | Canonical response lifecycle | Compatible | The reviewed lifecycle projection is equal. |
| `cpa-configuration-authority` | CPA `v7.2.80` | Configuration authority | Intentional | `BL-09`: the gateway owns versioned SQLite control-plane snapshots rather than reference file-watcher authority. |
| `grok2api-provider-pool-isolation` | grok2api `v3.0.0` / `ec6cddca7` | Build/Web provider-pool isolation | Compatible | Build/Web pool and browser-egress-bound Conversation semantics agree. |
| `grok2api-web-tool-default` | grok2api `v3.0.0` / `ec6cddca7` | Web Tool Emulation default | Intentional | `BL-20`: Web Tool Emulation is explicitly default-off until a dedicated gate permits it. |
| `kiro-rs-endpoint-policy` | Kiro-RS `c49c75e` | CLI/IDE endpoint policy | Compatible | The semantic endpoint-policy projection agrees. |
| `kiro-rs-event-stream-integrity` | Kiro-RS `c49c75e` | EventStream integrity | Compatible | CRC validation and chunk-invariant Canonical event semantics agree. |

The corpus has four `Compatible` and two `Intentional` entries.
It contains zero accepted `Regression` entries and no unclassified difference.

## Verification and review

| Check | Result |
|---|---|
| `cargo test --locked -p gateway-core --test p11_01_differential_fixtures` | PASS — committed corpus plus missing-classification, regression, forbidden-body-like field, unknown marker, source/subject, and subject/marker rejection paths. |
| `cargo clippy --locked -p gateway-core --test p11_01_differential_fixtures -- -D warnings` | PASS. |
| `cargo fmt --all -- --check`, `git diff --check` | PASS. |
| Source policy and crate boundaries | PASS — test-only code adds no runtime coupling or crate dependency. |
| `./scripts/check.sh docs` | PASS — plan state (one `IN_PROGRESS` before closeout, then zero), document links, tracked Secret scan, and whitespace checks. |
| Focused review | PASS — reviewed absence of runtime/network/credential/reference-source coupling, closed fixture vocabulary, value-free errors, strict equality/decision checks, and fail-closed regression behavior. |

## Remaining boundary

P11-01 does not prove a live reference implementation, provider quota/account state, P7/P8 deferred E2E, fault tolerance, performance, security, migration, recovery, or release readiness.
Those remain P11-02 through P11-08 work.
P11-02 and P11-03 may begin only after this task's reviewed local evidence is committed and P11-01 becomes `DONE` in the plan.
