# P12-08E4 Kiro unified runtime

## Result

P12-08E4 is `LOCAL_PASS_PENDING_PHASE_GATE`. The existing native Kiro adapter is now complete at
the P12 composition root: one encrypted runtime lease may contain the backward-compatible raw
headless API key or one strict, unexpired Social/Enterprise OAuth JSON object. CLI and IDE endpoint
policies, profile resolution, native AWS EventStream decoding, Tool/Thinking semantics, and exact
Credential/Quota/Endpoint Health ownership all remain provider-specific behind the common
Canonical runtime. No server, external OAuth, real credential, Config Version, listener, or
production traffic changed.

## Reference and porting boundary

CLIProxyAPI v7.2.101 commit `42a00a2a6521b867c27f7ad096d08699db8e6d19` has no Kiro executor to
port. It remains the reference only for the common Provider registry, request lifecycle, retry, and
stream-bootstrap shape. Kiro-specific behavior comes from the previously frozen Kiro-RS
`c49c75e` clean-room projections and the already-reviewed `provider-kiro` implementation:

- P11-01 fixes the CLI/IDE endpoint-policy and EventStream-integrity projections;
- P7-01 through P7-08 implement strict credential families, derived endpoints, profiles,
  conversation requests, EventStream framing, dynamic catalog, Tool/Thinking, and failure classes;
- P7-09's injected native adapter proves one-send IDE and CLI vertical slices without depending on
  Kiro-RS as a runtime layer.

CPAR intentionally does not copy ambient credential discovery, foreground file reads, raw response
diagnostics, hidden retries, or unbounded EventStream recovery. The shared DNS-pinned egress,
encrypted lease, immutable Route Snapshot, bounded decoder, and Router state registries remain
authoritative.

## Credential and refresh boundary

The request-time importer accepts exactly two storage forms:

| Lease form | Credential family | Runtime behavior |
|---|---|---|
| raw `ksk_` value | API key | accepted for backward compatibility; never refreshed |
| strict tagged JSON | Social or Enterprise OAuth, and canonical API-key JSON | duplicate-free exact fields; absolute expiry checked before egress |

An arbitrary non-JSON string cannot be coerced into an API key. Mixed families, unknown fields,
invalid Regions, malformed values, and expired OAuth fail before profile resolution or network
admission. The request path uses an already-unexpired access token only; it does not refresh,
discover a cache, or persist a replacement. P12-08F1 owns composition of the existing
singleflight/CAS refresh coordinator with durable encrypted replacement.

## Endpoint, profile, and protocol boundary

- `kiro.messages` remains a second implementation of `anthropic/messages`; Kiro does not invent a
  fourth client protocol.
- The stored HTTPS host/path must exactly equal the endpoint derived from `KiroEndpointKind` and a
  validated AWS Region. Foreign hosts, crossed CLI/IDE paths, plaintext, and appended path segments
  fail composition before a Secret is read.
- API keys omit `profileArn`; Social uses the frozen Builder profile; Enterprise accepts only a
  validated lookup snapshot or the reviewed deterministic Region-family fallback. Inference never
  performs hidden profile I/O.
- The fixed host-independent server environment projection avoids leaking the real working
  directory or OS. Each request identity creates a fresh conversation identity.
- Anthropic's required `max_tokens` ceiling is the one documented semantic Kiro cannot express and
  is intentionally dropped only for this adapter. Every other extension remains fail-closed.

## Stream and state ownership

The native adapter preserves the reviewed AWS EventStream prelude/message CRC checks, arbitrary
chunk boundaries, bounded corruption recovery, Text/Reasoning/Tool lifecycle, Plan Mode behavior,
and one terminal stream failure after semantic delivery.

Pre-start classifications keep one Router owner:

| Kiro result | Router handoff | Exact owner |
|---|---|---|
| credential unauthorized | non-retryable typed error | selected Endpoint/Credential health |
| independently proven account forbidden | non-retryable typed error | selected Endpoint/Credential account health |
| explicit quota exhaustion or rate limit | rate-limited attempt | selected Endpoint/Credential quota target |
| egress/transient failure | retryable connection class | selected Endpoint health/candidate failover |
| unknown 403 or unsupported semantics | non-retryable safe error | no credential/quota mutation |

The E4 correction adds `CredentialQuotaExceeded` to the rate-limited handoff. Previously that
already-classified Provider result became a generic non-retryable failure at the composition root,
so the exact Router quota registry could not observe it.

## Parity classification

| Behavior | Classification | Evidence |
|---|---|---|
| CLI/IDE fixed endpoint, headers, origin, profile, EventStream | `PARITY` | frozen Kiro-RS projections and provider suites |
| Text, Reasoning, Tool, Plan Mode, terminal semantics | `PARITY` | native adapter and Claude Code compatibility fixtures |
| raw API-key plus strict OAuth lease import | `PARITY` | all three provider credential families reach the same runtime boundary |
| DNS-pinned egress, strict JSON, bounded CRC recovery, no ambient discovery | `INTENTIONAL_HARDENING` | provider and gateway fail-closed regressions |
| hidden request-time OAuth refresh/profile lookup | `UNSUPPORTED_FAIL_CLOSED` | expired OAuth stops locally; F1 owns explicit workers/snapshots |
| live Kiro account success | `UNSUPPORTED_FAIL_CLOSED` for this slice | remains in deferred P7-09/P12-08G2 external-auth package |

## Verification and review

The following local evidence passed:

- `cargo test --locked --offline -p provider-kiro` — 44 tests passed across credentials, CLI/IDE
  policy, profiles, conversation requests, EventStream, catalog, Tool/Thinking, failures, and native
  adapter execution;
- `cargo test --locked --offline -p gateway runtime::tests --no-fail-fast` — 66 passed, including
  fixed Kiro endpoint admission, request projection, registry binding, and exact failure handoff;
- affected-package Clippy passed with warnings denied; and
- `./scripts/check.sh full` passed at task closeout.

Review retained the existing provider implementation rather than duplicating it in the gateway.
The only runtime changes are a strict dual-form lease importer and the missing exact quota handoff.
The user's untracked Kiro rotation helper remains outside the commit.

## Remaining boundary

P12-08F1 next composes the immutable multi-channel production graph and capability ledger. It must
include only locally passed channel/protocol pairs, wire explicit refresh/profile snapshots, and
leave channels without external credentials disabled-at-boot. P12-08G2 still owns Kiro live OAuth
evidence; E4 does not claim the currently unavailable external account is usable.
