# CPAR — Provider-Isolated Rust AI Gateway

[简体中文](README.zh-CN.md) | English

CPAR implements CPA/CLIProxyAPI gateway behavior in Rust for self-hosting operators.
Protocol, account, model and routing changes start from a pinned CPA Go implementation and
map into the existing Rust modules. Channels absent from CPA use their dedicated references.

> **Current entry point (2026-10-23):** use the [current status and development order](docs/engineering-roadmap.md)
> for implemented features, production wiring, qualification, the pinned CPA version and next work.
> The [architecture map](docs/03-target-architecture-draft.md) locates code; the
> [candidate inventory](docs/support/first-release-candidates.md) preserves client/release evidence.
> The old Prism frontend is retired; a future UI will be designed from the management API.

## Why CPAR

- **Provider isolation:** credentials, health, quota, sessions, clearance and egress state never
  silently cross Provider or Channel boundaries.
- **Protocol normalization:** Chat Completions, Responses and Anthropic Messages share a bounded
  canonical request/event model without erasing Provider-specific capability checks.
- **Fail-closed routing:** immutable route snapshots, exact credential leases, explicit revisions,
  bounded retries and first-semantic-event rules prevent accidental fallback.
- **Protected operations:** a separate loopback management listener provides versioned
  configuration, encrypted credentials, audit, billing and runtime projections.
- **Deployment discipline:** secrets stay outside the repository; production listeners remain
  loopback-only behind an operator-owned TLS reverse proxy.
- **Evidence-driven delivery:** phase reports, contracts, ADRs, traceability, local gates and
  immutable GitHub delivery tags are retained in the repository.

## Public API surface

| Capability | Endpoint | Notes |
|---|---|---|
| Model catalog | `GET /v1/models` | Authenticated and generated from the immutable route snapshot used for inference. |
| OpenAI Chat Completions | `POST /v1/chat/completions` | JSON and bounded SSE where the selected Provider/Channel declares support. |
| OpenAI Responses | `POST /v1/responses` | JSON/SSE projection with canonical lifecycle validation. |
| Responses WebSocket | `GET /v1/responses` | Strict `response.create`; one active plus one queued turn; not the Realtime API. |
| Stored response retrieval | `GET /v1/responses/{id}` | Exact Client-Key ownership; foreign/expired/deleted/missing IDs share a safe not-found result. |
| Stored response deletion | `DELETE /v1/responses/{id}` | Deletes only the exact Client-Key-owned stored response. |
| Response compaction | `POST /v1/responses/compact` | Gateway-owned bounded continuity token, on explicitly capable routes only. |
| Anthropic Messages | `POST /v1/messages` | JSON and bounded SSE projection. |
| Anthropic token count | `POST /v1/messages/count_tokens` | Route exists; the current production composition uses an unsupported counter. Counting implementation and wiring remain pending. |

All inference routes require a CPAR Client Key. Protocol compatibility never authorizes an
undeclared Provider feature: unsupported capabilities are rejected before Provider I/O.

### Supported Provider patterns

CPAR models capabilities instead of treating every upstream as interchangeable:

- generic OpenAI-compatible or Anthropic-compatible endpoints using an operator-owned
  `base_url + api_key` binding, including Krill-style relays;
- official Codex/ChatGPT credentials imported from supported CPA/Sub2API JSON envelopes or an
  operator-completed OAuth flow;
- Claude and Kimi authorization/import/refresh paths using the compatible protocol adapters;
- Grok Build and Grok Console account pools with Provider-specific credential/runtime state;
- Grok Web and Kiro adapters within their explicitly documented local/external evidence boundaries;
- additional compatible endpoints only after their adapter, protocol and egress capabilities are
  declared in the selected Config Version.

A credential format does not decide routing or proxy behavior. The exact Config Version,
Upstream, Endpoint, adapter, Credential binding, capability and egress policy do.

## Management and operations plane

The management listener is independent from the public data listener and must remain loopback-only.
It serves the versioned `/admin` API. The old frontend is retired; management remains available
through the API and local CLI.

Implemented management capabilities include:

- Config Version draft, validation, publication, rollback and revision/ETag workflows;
- Upstream, Endpoint, Credential, binding, route, candidate, alias, access-group and Client-Key
  management;
- encrypted credential import, OAuth workflow, metadata projection and reviewed export formats;
- configured account-pool inventory and Provider-owned runtime account-pool status;
- exact account operator actions and value-free failure feedback;
- runtime availability, quota recovery and Provider-scoped route explanation;
- usage aggregation, immutable price catalogs, billing materialization and routing-price policy;
- compatible egress pools, encrypted proxy nodes and exact binding profiles;
- Provider-specific egress/session/clearance status as separate source-domain rows;
- audit, observability, backup preflight and fail-closed restore staging.

Management responses use closed schemas and bounded pagination. They do not expose endpoint URLs,
credential plaintext/ciphertext, API keys, OAuth/SSO material, cookies, request bodies, raw Provider
errors or Client-Key digests.

## Architecture

```text
Native / CLI / server client
       │
       ├── OpenAI Chat Completions
       ├── OpenAI Responses HTTP JSON/SSE
       ├── OpenAI Responses WebSocket
       └── Anthropic Messages
                   │
                   ▼
        Authentication + access group
                   │
                   ▼
       Canonical request/event boundary
                   │
       ┌───────────┼────────────────────┐
       ▼           ▼                    ▼
 immutable     Provider-scoped      capability +
 route graph   credential lease     egress admission
       └───────────┼────────────────────┘
                   ▼
          Provider-specific adapter
                   │
                   ▼
          bounded upstream transport
                   │
       ┌───────────┴─────────────────┐
       ▼                             ▼
 canonical downstream events   async value-free events
                                     │
                                     ▼
                              SQLite + operations views
```

The Cargo workspace is split into focused crates for canonical types, protocols, routing,
credential/runtime state, transport, observability, encrypted persistence and Actix HTTP
composition. Core Provider/protocol logic does not depend on Actix request types. SQLite is not
consulted for ordinary request-time route selection; the data plane uses compiled immutable state.

## Security model

CPAR assumes that Provider credentials, account cookies, OAuth tokens and production databases are
high-value secrets.

1. Five deployment bootstrap credentials are supplied as direct regular files, never environment
   variables or command-line values.
2. Provider credentials and protected runtime payloads are sealed with domain-separated AEAD and
   revision/owner-bound associated data.
3. Public inference and protected management use different loopback listeners.
4. Management Key, same-origin/CSRF policy, Config Version identity and revision checks protect
   control-plane operations.
5. Provider, Upstream, Endpoint, Credential, account and egress ownership are independently
   validated before lease or transport.
6. Logs, audit rows, errors, cursors and Debug output are designed to be value-free.
7. Default tests do not contact real Providers, deploy servers or register accounts.

Read [SECURITY.md](SECURITY.md) before operating real credentials. Report suspected exposure through
a private GitHub Security Advisory, not a public issue.

## Quick start for developers

### Prerequisites

- Rust `1.97.1` (pinned by `rust-toolchain.toml`);
- Linux build packages: `build-essential`, `clang`, `cmake`, `libclang-dev`, `libssl-dev`,
  `pkg-config`, `ca-certificates`;
- macOS: current Xcode Command Line Tools and Homebrew/OpenSSL where required;
- `ripgrep`; optional `cargo-deny`, `cargo-audit` and `cargo-cyclonedx` for the full gate.

### Build

```bash
git clone https://github.com/Ricardo121380/cpa-rust-gateway.git
cd cpa-rust-gateway
cargo build --locked --release --package gateway
./target/release/gateway --help
```

The backend builds independently, without Node/npm or a frontend source tree.

### Verification

```bash
cargo fmt --all -- --check
cargo test --locked --workspace --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
./scripts/check.sh docs
```

For an integration or release candidate:

```bash
./scripts/check.sh fast
./scripts/check.sh full
```

The full gate installs or requires pinned supply-chain tools and is intentionally more expensive
than normal development checks.

## Deployment

The complete deployment instructions are separate from this README:

- [Deployment Guide — English](docs/deployment-guide.en.md)
- [部署指南 — 简体中文](docs/deployment-guide.zh-CN.md)

| Method | Intended environment | Architecture status |
|---|---|---|
| Source-built OCI/Docker image | Linux Docker with host networking | Linux `amd64`, `arm64` |
| Docker Compose | Single-node Linux, persistent bind mounts, host reverse proxy | Linux `amd64`, `arm64` |
| Native binary + systemd | Debian/Ubuntu and compatible systemd distributions | Linux x86-64, AArch64 |
| Native foreground build | Development and local evaluation | Linux and macOS host architecture |
| WSL2 | Windows development/evaluation | Linux x86-64 inside WSL2 |

There is currently no public GHCR image or public GitHub Release binary. The signed release
workflow produces private, short-lived artifacts for approved revisions. Public users should use
the source-build Dockerfile or compile natively until a public-release workflow is approved.

### Runtime bootstrap contract

`gateway serve` requires two distinct nonzero loopback listeners, an absolute writable state
directory and an absolute read-only credential directory:

```text
gateway serve \
  --data-listen 127.0.0.1:18180 \
  --management-listen 127.0.0.1:18181 \
  --state-dir /var/lib/cpa-rust-gateway \
  --credential-dir /run/cpa-rust-gateway-credentials
```

The credential directory must contain exactly named direct regular files:

| File | Format |
|---|---|
| `management-key` | `mgmt_`-namespaced ASCII, 32–512 bytes |
| `management-csrf` | independent `csrf_`-namespaced ASCII, 32–512 bytes |
| `master-key` | exactly 32 raw bytes |
| `backup-key` | exactly 32 raw bytes |
| `client-key-pepper` | exactly 32 raw bytes |

Never expose the management listener through Caddy, Nginx, a cloud load balancer or Docker port
publishing. Expose only the data listener through an HTTPS reverse proxy after configuring routes
and Client Keys.

## First configuration

A fresh process creates/opens `control.sqlite3` but has no inference routes. Operators must create
a draft Config Version, configure egress policy, Upstream, Endpoint, Credential binding, public
model, route/candidate, access group and Client Key, validate the graph, publish it, and restart the
runtime when using the offline CLI. Publishing through the running management API prepares and
atomically activates a new runtime generation.

Use the protected management API or local `gateway admin` commands. Maintain an external,
secret-free ledger of every opaque ID; some low-level resources intentionally have no collection
endpoint. The detailed order and rollback rules are documented in
[the P12 rollout runbook](docs/p12-rollout-runbook.md).

## Dynamic upstream model catalog

For supported discovery channels, `/v1/models` is not a handwritten Provider or subscription-tier
table. CPAR discovers exact upstream model IDs per active Config Version + Endpoint + Credential,
retains the last successful result in SQLite and atomically derives Credential-restricted runtime
routes from the published route template. Current runtime discovery sources include Grok Build,
official Codex, Kiro and compatible endpoints configured with `models_path`. Grok Web/Console
and xAI Official do not automatically inherit those discovery implementations. A source being
wired does not establish live acceptance for every credential or endpoint.

The default lifecycle is Fresh for 6 hours, refresh due after 24 hours and hard expiry after 72
hours. A transient failure preserves the last success. Removing a model requires three successful
omissions and at least 24 hours of isolation; failed calls do not count. The worker checks state
hourly but contacts a Provider only when a target has no success or its refresh deadline is due.

Operators can inspect the protected `GET /admin/catalog/status` resource. It reports exact opaque
Endpoint/Credential IDs, freshness, target-local snapshot version, whether refresh is due, retained
model count, and a bounded failure time/class. It deliberately does not return model IDs, URLs,
response bodies, account identities or secrets. After startup or upgrade:

1. authenticate `GET /v1/models` with the intended CPAR Client Key;
2. confirm only authorized exact upstream IDs are present;
3. make one bounded request using the chosen exact ID and the channel's real protocol;
4. use the protected status resource—not frontend constants—to diagnose missing/expired catalogs.

Account tiers such as Grok Build `free/supergrok/heavy` and ChatGPT
`free/go/plus/pro5x/pro20x` are separate entitlement metadata. They never generate model IDs.
Interactive first OAuth and recovery of a revoked refresh grant remain operator/Autoreg work;
routine refresh of a valid OAuth grant already imported into a supported CPAR channel is owned by
CPAR.

## Example client request

After an operator has published a route and issued a CPAR Client Key:

```bash
curl --fail-with-body https://your-cpar.example/v1/responses \
  -H 'Authorization: Bearer <CPAR_CLIENT_KEY>' \
  -H 'Content-Type: application/json' \
  -d '{"model":"<PUBLIC_MODEL>","input":"Reply with OK.","stream":false}'
```

For WebSocket mode, upgrade `GET /v1/responses` without a browser `Origin` header and send one
strict `response.create` JSON message. This downstream WebSocket is not Provider-native transport
and not the OpenAI Realtime API.

## First-release client selection (2026-10-23)

Claude Code **D2 is selected** for the first release, alongside Codex and Pi; all three must
complete the agreed Grok Build gates before READY. Use the complete version-bound
[D2 local configuration](docs/support/grok-build-messages.md) and
[candidate support inventory](docs/support/first-release-candidates.md).
The actual-client controlled result is historical PASS; fixed Build qualification remains
BLOCKED, #66 is OPEN and real Provider acceptance is NOT_RUN. Original Claude adaptive/display,
overall effort and block cache obligations remain open follow-up work. See
[ADR-0123](docs/adr/ADR-0123-first-release-claude-d2-restricted-support.md).

ADR-0125 request compatibility is now implemented locally; historical original-Claude rejection
counts are not current expectations. D2 remains selected, while actual stock-client and fixed-service
acceptance must be rerun for the current source. See the [current roadmap](docs/engineering-roadmap.md).

## Development status

Implementation, production wiring, evidence gaps and the next proposed slice are maintained in
[the current roadmap](docs/engineering-roadmap.md). The completed backend cleanup and frontend
retirement are described in [the architecture map](docs/03-target-architecture-draft.md).

<details>
<summary>October 18 and earlier development snapshots</summary>

As of 2026-10-18, the inspected checkout is `3369c730`. The gateway and management workflows exist;
[existing-channel convergence #9](https://github.com/Ricardo121380/cpa-rust-gateway/issues/9)
remains open. GitHub has 14 open issues, with 11 open direct prerequisites of
[#57](https://github.com/Ricardo121380/cpa-rust-gateway/issues/57).

- **Latest local gate:** the October 17 receipt binds `2f43057c`: Full 45/45 PASS; Rust 1515 PASS,
  0 FAIL, 13 IGNORED. This was not rerun today and does not qualify live channels or a new candidate.
- **Next iteration direction:** [#47](https://github.com/Ricardo121380/cpa-rust-gateway/issues/47)
  starts with Grok Build → Responses → multi-turn tool calls. The first usable version must pass
  Codex, Claude Code and Pi acceptance, all using Grok Build, including the Messages path.
  [ADR-0107](docs/adr/ADR-0107-standard-response-metadata-separation.md) accepts reviewed metadata
  separation with complete encrypted retention. Field admission and implementation are pending;
  current strict behavior still rejects the complete sample's extra Usage.
  [ADR-0108](docs/adr/ADR-0108-build-summary-standard-messages.md) separately allows Claude Code
  to omit displaying Grok summaries, provided encrypted retention and tool continuity are proven;
  this is not implemented yet.
- **Versions and extensions:** deliver versions with explicit support scopes while retaining all
  unfinished #9 commitments. The accepted `native-v1` direction proceeds separately; the profile,
  dedicated product consumer and complete durable evidence remain unimplemented.
- **Remaining blockers:** Kiro native caps, Grok Web continuation and some caps, live channel
  evidence and candidate-specific delivery conditions. See the
  [October 18 roadmap](docs/reports/engineering-roadmap-20261018.md) for evidence and scope. Production runtime
  state was not inspected in this documentation iteration.

<details>
<summary>Historical P13 status, retained at its original evidence scope, not the current task queue</summary>

- **Formally gated implementation:** approved backend slices through P13-11E4, including P13
  management/billing/routing, Channel Pin, stored Responses, Responses WebSocket, compatible egress
  pools and Provider egress status projection.
- **Frontend integration:** Prism evolves independently against the generated management contract;
  check the current branch and `docs/cross-boundary-log.md` for pending handoffs.
- **In progress:** P13-15 all-channel upstream model-catalog pass-through. Exact-Credential Build
  and Codex sources have observed `grok-4.6`, `grok-4.5`, `gpt-5.6-terra`, `gpt-5.6-luna`,
  `gpt-5.5` and `gpt-5.4-mini`; P13-15C/D durable freshness/removal, protected status and atomic
  Credential-scoped route materialization now pass in Oracle Singapore for Grok Build:
  authenticated `/v1/models` dynamically includes `grok-4.6`, and one real non-streaming Responses
  canary completed. Remaining channel sources, target ChatGPT Go/Codex evidence, the isolation
  matrix, Claude Code integration and the formal gate are still pending, so clients must consume
  the gateway list rather than manual constants.
- **Explicitly deferred or externally blocked:** real Kiro/Official API-key E2E, Grok Web external
  egress/WAF evidence, P13-11E5 real Provider/proxy/DNS canary, automatic account registration or
  repair, media/files/batch and additional Providers.
- **CPAR credential lifecycle:** imported, bound OAuth grants with an explicit Provider refresh
  protocol are renewed by CPAR at startup and during service operation, persisted through encrypted
  CAS and atomically published to later runtime leases. API keys and SSO cookies are not treated as
  refreshable OAuth. P13-16A has proved Grok Build automatic refresh and continued serving in
  production; invalid Codex grants use bounded `1/2/4/.../60` minute backoff. Claude and Kiro need
  their own exact-channel executor before production activation.
- **Not part of CPAR:** Autoreg account registration, initial login/authorization, interactive
  reauthorization after a refresh grant is revoked, entitlement repair and replenishment. Autoreg
  is not involved in routine refresh of OAuth material already stored by CPAR.

`DONE_WITH_BOUNDARY` means the documented acceptance boundary passed; it does not claim that every
Provider account, external network path or production deployment was tested.

</details>

</details>

## Git and release governance

Development uses phase/integration branches and immutable `phase-p*-complete` evidence tags.
Ordinary branch pushes and PR updates run lightweight checks; the expensive delivery gate is
explicit and revision-bound. A dated inventory of every current branch and its merge recommendation
is available in [the Git branch audit](docs/git-branch-inventory-2026-08-19.md).

The safe integration path is one reviewed integration PR into `main`, followed by one formal gate
for the immutable final revision. Historical phase branches that are already ancestors do not need
to be merged again; non-ancestor branches must be reconciled deliberately rather than blindly
merged.

## Repository map

| Path | Purpose |
|---|---|
| `apps/gateway` | CLI, process composition, data/management listeners |
| `crates/gateway-*` | core, auth, catalog, control, routing, storage, transport, HTTP and observability |
| `crates/protocol-*` | downstream/upstream protocol codecs |
| `crates/provider-*` | Provider-specific adapters and state |
| `docs/adr` | accepted architecture decisions |
| `docs/contracts` | executable behavior/security contracts |
| `docs/reports` | phase and verification evidence |
| `deploy` | systemd, Caddy and Docker deployment assets |
| `scripts` | deterministic checks, release verification and bounded operator helpers |

## Documentation

- [Current status, CPA baseline and development order](docs/engineering-roadmap.md)
- [Deployment Guide](docs/deployment-guide.en.md)
- [Backend completion audit](docs/backend-completion-audit-2026-08-19.md)
- [Git branch inventory](docs/git-branch-inventory-2026-08-19.md)
- [Behavior contracts](docs/02-behavior-contracts.md)
- [Target architecture](docs/03-target-architecture-draft.md)
- [Channel reference analysis](docs/04-channel-reference-analysis.md)
- [Historical development plan and technical baselines](docs/06-development-plan.md)
- [Historical management frontend plan](docs/08-management-frontend-development-plan.md)
- [Traceability](docs/traceability.md)
- [Architecture decisions](docs/adr/README.md)
- [Contracts index](docs/contracts/README.md)
- [Reports index](docs/reports/README.md)
- [Quality gates](docs/quality-gates.md)
- [Crate boundaries](docs/crate-boundaries.md)
- [Third-party notices](THIRD_PARTY_NOTICES.md)

## Contributing

Keep changes small, Provider-scoped and evidence-backed. Never commit real credentials or raw
Provider payloads. Update the authoritative OpenAPI contract before generated clients, preserve
the module boundaries in `AGENTS.md`, add an ADR/contract for new security semantics, and
include focused tests plus a value-free verification receipt.

## License

CPAR is licensed under the [MIT License](LICENSE). You may use, copy, modify, merge, publish,
distribute, sublicense and sell copies subject to retaining the copyright and permission notice.
Reference-project attribution and license notes remain in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Controlled Grok Build Messages slice (#63)

The standard Grok Build Messages path does not display reasoning summary. Confirmed summary is
retained as encrypted diagnostics before successful JSON/SSE closure. The controlled scope uses
explicit standard text/tool history; actual Claude Code, real Provider and #61 field/profile
qualification remain separate. See [support scope](docs/support/grok-build-messages.md).
