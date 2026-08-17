# P13-11D compatible proxy-pool management report

Status: `P13-11D LOCAL_PASS_PENDING_PHASE_GATE`; D1, D2, and D3 are locally implemented and reviewed

Date: 2026-08-17

## Outcome first

P13-11D is a new post-Gate task. D1, D2, and D3 are now implemented as local, reviewable slices;
no formal D Delivery Gate is claimed. P13-11A/B/C remain formally accepted by
`phase-p13-egress-complete` at `a716eaaa9d31c26b6d09489f3f7fdbb9b0e1ebeb` and GitHub run
[31959162202](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31959162202).
The old tag will not be moved or reinterpreted to include D.

The former runtime gap is closed locally: `apps/gateway` now opens enabled compatible node
endpoints exactly once while composing the active Config Version, builds one Upstream-owned
Direct/fixed/pool transport registry and exact Endpoint-Credential binding settings, then reuses
the existing P13-11B/C lease and serving path. The request hot path does not read the Store or
decrypt node material.

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

Delivered locally in commit `edf1f6f` (not formally gated):

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

Implemented locally in the current worktree (not formally gated):

- `gateway-control` opens an enabled node only with the exact
  Config-Version/Upstream/pool/node AAD tuple and immediately re-parses the authenticated plaintext
  into the redacted local-DNS SOCKS5 runtime type;
- `apps/gateway` compiles enabled generic compatible Upstreams into the existing Direct/fixed/pool
  registry and compiles durable profiles into exact Endpoint-Credential runtime settings;
- enabled resources owned by native/non-compatible Upstreams, foreign or disabled pool members,
  empty enabled pools, wrong AAD, unsafe proxy plaintext, unknown targets, duplicate binding
  settings, and standalone weighted nodes fail before runtime publication;
- no durable compatible configuration preserves the Direct-only default; disabled draft rows are
  inert; serving continues through the existing P13-11C exact Credential/egress lease path;
- node secrets are opened only during composition. No request-path Store read, decrypt, ambient
  proxy lookup, DNS lookup, or client-directed target selection was added.

D3 does not authorize or claim a real proxy, DNS path, or Provider request.

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
  `seal_compatible_proxy_node_endpoint` plus the exact-AAD
  `open_compatible_proxy_node_endpoint`; both reuse `UpstreamProxy::try_socks5`, and opening never
  returns plaintext as a string.
- `apps/gateway::runtime` compiles the active durable pool/node/binding graph into the existing
  `CompatibleEgressTransportRegistry` and `CompatibleEndpointBindingRuntimeSettings` inputs.
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

Local D3 checks passed:

- `gateway` 109/109, `gateway-control` 77/77, `gateway-router` 151/151, and
  `gateway-upstream` 37/37 tests passed;
- focused D3 runtime tests cover a two-node weighted pool, bounded capacity and lease release,
  exact durable binding settings, the no-configuration Direct default, enabled empty pool,
  foreign native Upstream ownership, orphaned exact binding profiles, and wrong Config-Version
  AAD rejection;
- the control-plane open test covers exact AAD success, wrong-Version authentication failure,
  authenticated unsafe plaintext rejection, and redacted `Debug` output;
- strict Clippy for `gateway` and `gateway-control` with all targets/features and `-D warnings`,
  workspace formatting, docs checks, secret scan, and diff checks passed;
- no management OpenAPI or `web/prism/**` surface changed in D3. The existing D2 action-required
  Claude Code handoff remains the only frontend integration notice.

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
- a real proxy/Provider connectivity result, aggregate Full, or formal P13-11D Delivery Gate
  completion.

## Next action

Next action: commit the D3 runtime slice, then perform the P13-11D aggregate local Full/review
closeout. Only after that evidence is complete may one new exact closeout commit/tag run the single
formal P13-11D Delivery Gate. Do not move or reinterpret `phase-p13-egress-complete`, and do not
call Provider, proxy, DNS, server, staging, or production systems for this local closeout.
