# P13-11D compatible proxy-pool management report

Status: IN_PROGRESS (`P13-11D1` design frozen; implementation pending)

Date: 2026-08-17

## Outcome first

P13-11D has started as a new post-Gate task. Its architecture and acceptance boundary are frozen;
no implementation claim is made yet. P13-11A/B/C remain formally accepted by
`phase-p13-egress-complete` at `a716eaaa9d31c26b6d09489f3f7fdbb9b0e1ebeb` and GitHub run
[31959162202](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31959162202).
The old tag will not be moved or reinterpreted to include D.

The current runtime gap is precise: `apps/gateway` constructs one Upstream-owned compatible
transport registry with Direct enabled and empty `fixed_proxies`, `proxy_pools`, and durable
binding settings. P13-11B/C already know how to select and hold fixed/pool egress leases in
deterministic fixtures, but the Config Version cannot persist or manage those inputs.

## Frozen implementation slices

### P13-11D1 — typed persistence and AEAD

Planned output:

- migration for Config-Version-owned proxy pools, proxy nodes, and exact Endpoint-Credential
  binding profiles;
- typed `gateway-store::control_plane` records and deterministic graph load/write order;
- local-DNS SOCKS5 endpoint validation followed by immediate `SecretStore` sealing under an
  egress-node-specific AAD domain;
- draft revision, ownership, delete/restrict, clone/rollback/reopen, corruption/row-swap, key
  rotation, migration down/up, and secret-free diagnostics tests.

D1 does not change HTTP, the authoritative management OpenAPI, Prism, the serving composition, or
the Direct production default.

### P13-11D2 — protected management contract

Planned output:

- bounded list/get/create/update/rotate/delete operations for pool, node, and exact binding
  profile resources;
- existing Management Key, origin/CSRF, `X-Config-Version`, `If-Match`, draft-only revision, and
  value-free audit behavior;
- a response shape that exposes opaque identity, enabled state, weight/capacity and closed policy
  values, plus `proxy_configured`, but never proxy URL/ciphertext/key version;
- authoritative `docs/openapi/management-v1.json` update, generated Prism contract/client sync,
  HTTP/OpenAPI security regression, and a precise `docs/cross-boundary-log.md` handoff to Claude
  Code.

No formal Prism page is part of the backend slice.

### P13-11D3 — active runtime composition

Planned output:

- one startup/publication-time compiler that decrypts only the active graph and builds the
  existing P13-11B fixed/pool registry plus binding settings;
- exact same-Upstream ownership, enabled-node, non-empty-pool, weight/capacity/schedule, target,
  AAD, SOCKS5, and Config Version checks before publication;
- hot-path Store/decryption absence, Direct default preservation, and deterministic loopback tests
  through the existing P13-11C serving handoff.

D3 still does not authorize a real proxy or Provider request.

## Security and ownership decisions

- Pool and node state is Provider/Upstream-local. There is no global proxy pool.
- A node is either a standalone fixed target or a member of exactly one pool.
- A binding profile is keyed by an existing exact Endpoint-Credential binding and may reference
  only a target owned by that same Upstream.
- Proxy endpoint text is encrypted even without user-info and never returned after submission.
- Only local-DNS `socks5://host:port` is admitted in the first version. `socks5h`, HTTP/HTTPS,
  proxy authentication, paths, query, fragments, and remote-DNS proxying are out of scope.
- Client requests and credential format never choose an egress target.
- Credential Health/Quota/Circuit, egress-node state, and Provider-specific session/clearance state
  remain separate failure domains.

## Implementation evidence

PENDING. D1 code, migration, and tests have not yet been completed.

## Verification evidence

PENDING. The first verification target is the D1 Store/AEAD matrix; no Fast, Full, or formal
Delivery Gate is claimed by this report yet.

## Frontend handoff

D1 has no management contract change and therefore does not touch OpenAPI or Prism. D2 will be a
cross-boundary change. At that point Codex must update the authoritative OpenAPI first, run
`npm --prefix web/prism run sync-contract`, and add an action-required log entry naming every
contract/generated surface. Claude Code must consume the generated client and must not retain or
redisplay proxy endpoint input.

## Explicit non-evidence

This report is not evidence for:

- a real proxy, DNS path, provider endpoint, account, or external egress;
- Grok Web clearance, Console bootstrap, FlareSolverr, Kiro, or native-provider behavior;
- Autoreg registration, OAuth/SSO, refresh, account repair, or replenishment;
- server, staging, production, public API, management UI, or traffic changes;
- P13-11D local pass, aggregate Full, or formal Delivery Gate completion.

## Next action

Implement P13-11D1 migration and typed Store/AEAD model, run the bounded D1 test/review matrix, and
update this report with exact evidence before starting D2.
