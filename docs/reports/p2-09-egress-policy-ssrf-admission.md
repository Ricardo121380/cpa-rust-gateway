# P2-09 EgressPolicy SSRF admission report

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P2-09` |
| Matrix / behavior | `L36`, `E10`, `J19`; Behavior 20 |
| Date | `2026-07-19` |
| Branch | `codex/p2-09-egress-policy` |
| Rust | `1.97.1` |
| Result | PASS |

## Delivered scope

- Added version-scoped `egress_policies` storage in migration 3, a typed `EgressPolicyId`,
  structural JSON allowlists, redirect mode/bound, same-version Upstream references, safe rollback,
  and complete Repository round-trip coverage. Legacy non-null opaque IDs become deliberately empty
  policies: their ID is retained, but they cannot be published or create network access until an
  operator replaces them with a valid policy.
- Enforced reference integrity at the `SQLite` write boundary. Invalid and cross-version references
  are rejected; a referenced policy ID cannot be renamed; deleting a policy atomically clears its
  nullable references so no dangling reference persists and an enabled Upstream then fails closed
  at publication.
- Added immutable, transport-neutral `gateway-upstream::EgressPolicy`: exact canonical scheme,
  Host, and effective-port allowlists; normalized CIDRs; an injected DNS resolver; and an
  `AdmittedEgressTarget` that retains the fully checked address answer for one later dial attempt.
- Admission resolves every domain once per attempt and rejects an empty answer, resolver failure,
  or any denied answer. It blocks loopback, private/shared, link-local, cloud metadata, multicast,
  documentation, reserved, NAT64/6to4-related, and other special ranges by default. Only explicit
  loopback/RFC1918/ULA CIDRs can admit deliberate local testing; broad CIDRs cannot bypass those
  protections or admit metadata.
- Added bounded redirect policy: deny by default; same-origin and full-revalidation modes resolve
  relative Locations, re-run URL/DNS/CIDR admission, and enforce a positive maximum hop count.
- Added management-time `EgressPolicyCompiler`. `SnapshotPublicationService` now invokes it before
  Route compilation, SQLite activation, or `ArcSwap` publication. Thus an enabled Upstream without
  a valid policy, a dangling policy, malformed policy, or rejected Endpoint URL cannot replace the
  active Snapshot.
- Added [ADR-0009](../adr/ADR-0009-egress-policy-ssrf-admission.md) and
  [BC-SEC-002](../contracts/BC-SEC-002-egress-policy-ssrf-admission.md), and updated P2-01's
  historical documentation to distinguish its original opaque field from migration 3 enforcement.

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-core -p gateway-upstream -p gateway-store -p gateway-control` | PASS; 38 core, 9 EgressPolicy, 21 Store, 1 Repository integration, and 18 control tests |
| `cargo clippy --locked -p gateway-core -p gateway-upstream -p gateway-store -p gateway-control --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `ruby scripts/check-crate-boundaries.rb` | PASS; the only new direct dependency is `gateway-upstream -> url` |
| `ruby scripts/check-doc-links.rb` | PASS |
| `scripts/secret-scan.sh --all` and `git diff --check` | PASS |
| `./scripts/check.sh fast` | PASS; workspace format, Clippy, tests, source policy, secret scan, boundary, documentation, and whitespace gates |
| `./scripts/check.sh full` | PASS; Fast gate plus pinned-tool verification, `cargo deny check`, and `cargo audit` |

## Review

Review passed. It verified the policy is immutable and transport-neutral, does not expose a raw URL
through `Debug`, has no socket/HTTP/proxy/Repository dependency, and maps all admission failures to
safe egress-owned errors. The review strengthened the initial implementation with IPv6 literal and
special-range coverage, cloud-metadata hard denial despite a ULA allowlist, a DNS failure/empty
answer test, broad-CIDR regression coverage, deletion-without-dangling-reference behavior, and
legacy migration preservation.

It also found that a static compiler alone would leave publication too permissive. The final design
runs EgressPolicy compilation before existing RouteSnapshot publication and proves a rejected draft
leaves both the active Snapshot and durable Config Version unchanged. The later P3 connector must
carry this compiled policy in a secret-free runtime target view and dial only the returned pinned
addresses; it must not re-resolve or query the Repository on the request path.

## Scope and deferred work

P2-09 does not open sockets, configure TLS, select a proxy, implement an HTTP client/pool,
follow actual response redirects, perform provider retries, expose a management HTTP/CLI endpoint,
or send traffic to a real upstream. P2-10 owns management create/validate/publish/rollback
interfaces; P3 owns the runtime connection adapter, address-pinned dialing, timeout/proxy rules,
and redirect response loop. No P3 work has begun. All fixtures are synthetic and no deployed
Credential, Client Key, Pepper, Master Key, URL query secret, or production traffic was added.

## GitHub CI

GitHub Actions run [29685685002](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29685685002)
passed for implementation commit `2be6ea7`.

| Job | Result |
|---|---|
| Fast gate | PASS; completed `2026-07-19T11:46:09Z` |
| Full supply-chain gate | PASS; completed `2026-07-19T11:56:41Z` |

This completes P2-09 implementation acceptance. The verification-record commit below is pushed
separately and must also pass the same GitHub workflow before P2-10 starts.
