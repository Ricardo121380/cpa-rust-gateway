# P13-11B compatible-endpoint runtime composition report

Status: `DONE_WITH_BOUNDARY`

Date: 2026-08-16

## Intended outcome

Compose the P13-11A generic compatible-endpoint profile with the active Config Version, existing
Endpoint Credential pools, shared runtime Health/Quota registries, and a bounded Direct/local-DNS
SOCKS5 transport registry. Keep credential/account state separate from egress-node state and do
not contact Providers or the network.

## Frozen boundary

This is a local router/control composition slice. It does not switch the serving adapter, run a
proxy probe, perform Grok Web clearance, refresh credentials, use Autoreg, alter production or
server state, or add an OpenAPI/Prism/frontend surface.

## Implementation evidence

Implemented in `crates/gateway-router/src/compatible_egress_runtime.rs` and
`crates/gateway-router/src/compatible_endpoint_runtime.rs`, with the active-graph compiler in
`crates/gateway-control/src/compatible_egress_runtime_compiler.rs`. The transport registry is
owned by one Upstream identity, uses only Direct/local-DNS SOCKS5 profiles, and keeps node state
separate from Credential Health/Quota. Runtime construction is fail-closed for draft Config
Versions, foreign EgressPolicy snapshots, foreign transport registries, Credential revision or
schedule drift, mixed ownership, and unsupported/native adapter families.

## Verification

Local focused results: gateway-upstream diagnostic snapshot `1` passed; gateway-router compatible
runtime `11` passed; gateway-control compiler `6` passed. Full touched-crate results: upstream
`37/37`, router `149/149`, control `72/72`; strict Clippy for all three crates passed. The final
local Fast Gate `/tmp/cpar-p13-11b-fast-final.md` passed all listed steps, including Rust tests, P12
serve envelope, source/crate-boundary checks, docs, tracked-secret scan and whitespace. `git diff
--check` passed. No Provider, Store-at-request-time, DNS, socket, proxy or production operation was
performed.

## Review

Independent review fixed three issues before this receipt: transport-registry ownership was made
explicit per Upstream, active Config/policy/pool lineage is checked before publication, and node
availability is held through the capacity CAS so concurrent disable/cooldown cannot race a lease.
The final review found no remaining P1/P2 issue in this local slice. The four pre-existing
untracked helper scripts were preserved untouched; no OpenAPI/Prism/frontend surface changed.

## Next boundary

P13-11C may connect the composed transport assignment to request-time serving and add
Provider-specific sticky/probe/failure feedback only after this local composition is reviewed.

## Formal phase gate

P13-11B is accepted as part of the aggregate P13-11 closeout:

- immutable tag: `phase-p13-egress-complete`;
- exact commit: `a716eaaa9d31c26b6d09489f3f7fdbb9b0e1ebeb`;
- formal run: [31959162202](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31959162202);
- Authorize `success` (4s), Fast `success` (6m25s), Full supply-chain `success` (1m15s), and
  Required `success` (3s).

The Gate accepts deterministic local runtime composition and exact lease isolation. It does not
authorize a real proxy probe, Provider traffic, server deployment or the later protected
proxy-pool management surface.
