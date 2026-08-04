# P12-10H grok2api parity review

Status: `LOCAL_REVIEW_PASS_PENDING_STAGING_HTTP_E2E`

## Frozen behavior source

The behavior baseline is the read-only grok2api `v3.0.10` source at commit
`c27f0545197b3edf41d5deedcc2c3c3597887766`. CPAR does not run grok2api in the request path and
does not copy its database or ciphertext. The source is the normative behavior reference for the
native Grok account pool, not an invitation to invent a second routing policy.

## Source-to-CPAR mapping

| grok2api source behavior | CPAR implementation boundary | Review result |
|---|---|---|
| `application/gateway/selector.go`: authentication, model capability, account/model cooldown, quota recovery and bounded account lease gates | Native account rows compile once into the existing `EndpointCredentialPools`; the shared scheduler, `RuntimeHealthRegistry` and `RuntimeQuotaRegistry` perform request-time admission | PASS; no SQLite or refresh operation is added to the inference hot path |
| `application/gateway/selector_plan.go`: capability-aware priority, weighted capacity and deterministic tie-break planning | Native metadata preserves priority, weight and concurrency; a single native staging endpoint has no cross-route ordering ambiguity, while the existing pool keeps bounded priority/weight/lease semantics | PASS for the current one-endpoint staging slice; full-pool ordering remains a P12-10 parity gate |
| `infra/provider/cli/adapter.go`: fixed Build Responses request profile, one upstream send, strict JSON/SSE decode and explicit operation fallback rules | `provider-grok` Build request builder/decoder and injected upstream transport; transport has no implicit retry or fallback, and the staging route fixes `max_attempts=1` | PASS; no ad-hoc fallback was introduced |
| `infra/provider/cli/fallback.go`: xAI fallback is conditional on account/billing/mode and only after the eligible Build 403 | The native staging route exposes only the Build endpoint and has no xAI sibling candidate, so fallback is not silently synthesized; it remains disabled until an explicitly bound Official route is reviewed | PASS; fail-closed rather than invented fallback |
| `infra/provider/console/import.go`: JSON/plain-text account import extracts and sanitizes `sso_token` | CPAR accepts the raw bounded token and the migration helper's strict `{sso_token, probe_model}` envelope, extracts only the token for the cookie, and rejects unknown fields/invalid observed models | PASS; regression added in `p12_10e_console_web_runtime` |

The only implementation differences are Rust ownership/zeroization, CPAR's immutable config-version
publication, and the already-existing Canonical/protocol projection layers. They do not change the
provider request, eligibility, cooldown, quota, or fallback semantics above.

## Evidence and boundary

Local review and gates passed after the envelope correction:

```text
cargo fmt --all -- --check
cargo test --locked -p provider-grok --test p12_10e_console_web_runtime --test p12_10f_grok2api_memory_migration
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
./scripts/check.sh fast
```

The direct CPAR HTTP E2E remains blocked until an isolated staging config version contains a
native Grok route. The production graph, Caddy, DNS, old CPA, CC Switch and the stopped grok2api
service remain unchanged. The next step is to use the reviewed signed Linux artifact in that
loopback-only staging graph, run the bounded direct HTTP harness, and roll the graph back.
