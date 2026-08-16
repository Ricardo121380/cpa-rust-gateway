# P13-11D compatible proxy-pool management report

Status: `P13-11D2 LOCAL_PASS_PENDING_PHASE_GATE`; D1 and D2 are locally implemented/reviewable; D3 remains not started

Date: 2026-08-17

## Outcome first

P13-11D is a new post-Gate task. D1 and D2 are now implemented as local, reviewable slices;
no formal D delivery gate is claimed. P13-11A/B/C remain formally accepted by
`phase-p13-egress-complete` at `a716eaaa9d31c26b6d09489f3f7fdbb9b0e1ebeb` and GitHub run
[31959162202](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31959162202).
The old tag will not be moved or reinterpreted to include D.

The current runtime gap is precise: `apps/gateway` constructs one Upstream-owned compatible
transport registry with Direct enabled and empty `fixed_proxies`, `proxy_pools`, and durable
binding settings. P13-11B/C already know how to select and hold fixed/pool egress leases in
deterministic fixtures, but the Config Version cannot persist or manage those inputs.

## Frozen implementation slices

### P13-11D1 — typed persistence and AEAD

Delivered locally in commit `8849b9c`:

- migration for Config-Version-owned proxy pools, proxy nodes, and exact Endpoint-Credential
  binding profiles;
- typed `gateway-store::control_plane` records and deterministic graph load/write order;
- local-DNS SOCKS5 endpoint validation followed by immediate `SecretStore` sealing under an
  egress-node-specific AAD domain;
- draft-owned graph write order, same-Upstream ownership, delete/restrict triggers, corruption/
  row-swap/AAD-boundary, migration down/up, and secret-free diagnostics tests.

The existing draft revision/If-Match mutation service is reused by D2; D1 adds the typed graph
records and keeps `write_configuration` draft-only. Clone/rollback/reopen use the existing Store
graph machinery and remain covered by the repository's broader control-plane regression suite.

D1 does not change HTTP, the authoritative management OpenAPI, Prism, the serving composition, or
the Direct production default.

### P13-11D2 — protected management contract

Implemented locally in the current worktree (not formally gated):

- bounded list/get/create/update/delete operations for pool, node, and exact binding-profile
  resources under the existing protected `/admin/operations` scope;
- existing Management Key, peer/origin/CSRF, `X-Config-Version`, `If-Match`, draft-only revision,
  atomic mutation and value-free audit behavior;
- closed response DTOs exposing only opaque identity, enabled state, weight/capacity and closed
  target/failure/stickiness/retry values, plus `proxy_configured`; proxy endpoint input is
  write-only and update omission preserves the sealed value;
- authoritative `docs/openapi/management-v1.json` update, Prism contract/generated-client sync,
  OpenAPI operation-count regression and a precise `docs/cross-boundary-log.md` handoff to Claude
  Code. Successful revisioned responses now carry `Cache-Control: no-store`.

No formal Prism page is part of this backend slice. Claude Code action is required before any
management UI is built; generated artifacts must be consumed, not hand-edited, and proxy input
must never be displayed or retained by the frontend.

### P13-11D3 — active runtime composition

Planned output:

- one startup/publication-time compiler that decrypts only the active graph and builds the
  existing P13-11B fixed/pool registry plus binding settings;
- exact same-Upstream ownership, enabled-node, non-empty-pool, weight/capacity/schedule, target,
  AAD, SOCKS5, and Config Version checks before publication;
- hot-path Store/decryption absence, Direct default preservation, and deterministic loopback tests
  through the existing P13-11C serving handoff.

D3 is not started and does not authorize a real proxy or Provider request.

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

- `crates/gateway-store/migrations/0019_compatible_egress_pool.{up,down}.sql` adds three
  Config-Version-owned tables, composite same-Upstream foreign keys, target checks, and
  restrict/update triggers.
- `gateway-store::control_plane` now has typed pool/node/exact binding records, deterministic
  write/load order, bounded identities, capacity/weight/retry validation, and redacted Debug.
- `gateway-control::control_plane_service` exposes the versioned length-delimited node AAD and
  `seal_compatible_proxy_node_endpoint`, reusing `UpstreamProxy::try_socks5` before AEAD sealing.
- The four pre-existing untracked helper files were not staged or modified.

## Verification evidence

Local D1 checks passed:

- `cargo test --locked -p gateway-store`: 61 unit tests, repository/backup/upgrade integration
  tests, and doc tests all passed;
- `cargo test --locked -p gateway-control`: 75 tests passed; the focused proxy AAD/sealing slice
  is 6/6;
- `cargo check --locked -p gateway-control -p gateway` and
  `cargo check --locked -p differential-gate` passed;
- strict Clippy for `gateway-store` and `gateway-control`, workspace formatting, staged secret
  scan, and `git diff --check` passed.

Local D2 checks passed:

- protected HTTP round trip for pool/node/binding create, list, update, stale revision and delete;
  the focused `p10_04_management_resources` case passed and verified that endpoint text,
  ciphertext, and proxy fields are absent from responses;
- `gateway-store` 61/61 and `gateway-control` 76/76 compilation/tests and strict Clippy passed
  with the D2 mutation methods and bounded composite-audit-ID regression;
- `p10_01_management_openapi_contract` passed with 57 required operations and 31 protected
  writes;
- management JSON decoding now rejects duplicate object keys recursively in addition to unknown
  fields and bounded-value checks;
- `npm --prefix web/prism run sync-contract`, `npm --prefix web/prism run check`,
  `node web/prism/scripts/generate-client.mjs --check`, and
  `node scripts/check-management-spa.mjs` passed (98 generated operations);
- OpenAPI JSON validation, formatting, and diff checks passed. No Provider, proxy, DNS, server,
  staging, or production activity occurred.

This is a local slice result only. No Fast/Full or formal P13-11D Delivery Gate is claimed yet.

## Frontend handoff

D1 had no management contract change. D2 is the cross-boundary change: Codex updated the
authoritative OpenAPI first, ran `npm --prefix web/prism run sync-contract`, and added an
action-required log entry naming every contract/generated surface. Claude Code must consume the
generated client and must not retain or redisplay proxy endpoint input.

## Explicit non-evidence

This report is not evidence for:

- a real proxy, DNS path, provider endpoint, account, or external egress;
- Grok Web clearance, Console bootstrap, FlareSolverr, Kiro, or native-provider behavior;
- Autoreg registration, OAuth/SSO, refresh, account repair, or replenishment;
- server, staging, production, public API, management UI, or traffic changes;
- D3 active runtime composition, aggregate Full, or formal P13-11D Delivery Gate completion.

## Next action

Next action: complete the D2 local review and commit the backend/OpenAPI/Prism contract slice.
After review, start P13-11D3 active Config-Version runtime composition. Do not run a formal D
Delivery Gate until D3 and the aggregate evidence are complete; do not call Provider, proxy, DNS,
server, staging, or production systems.
