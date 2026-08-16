# P13-11A generic compatible-endpoint egress report

Status: `DONE_WITH_BOUNDARY`

Date: 2026-08-16

## Intended outcome

Create the first CPAR-side egress foundation for generic compatible endpoints. The implementation
must treat Krill, CPA relay, Sub2API relay, and any custom `base_url + api_key` endpoint as one
configuration pattern, while retaining explicit Provider/channel/credential isolation.

## Implementation evidence

* `gateway-upstream::CompatibleEndpointEgressProfile` binds one upstream, endpoint, credential,
  source label, wire protocol, and egress policy.
* `Direct`, `FixedProxy`, and `ProxyPool` are closed target modes; failure ownership, stickiness,
  and pre-submit retry are explicit and bounded.
* Endpoint/credential ownership is checked before composition. URL shape and static SSRF policy
  checks delegate to the existing `EndpointUrl` and `EgressPolicy` implementations.
* No URL, API key, OAuth token, cookie, proxy secret, body, or Provider response is stored in the
  profile or included in its debug/error projection.

## Local verification

* Five focused `gateway-upstream` tests pass: relay-name neutrality, CPA/Sub2API metadata
  neutrality, exact ownership and SSRF-policy reuse, invalid retry/proxy semantics, and bounded
  labels. The complete package suite passes `37/37` when run serially; one parallel run had a
  transient loopback transport race in an existing slow-stream test and the exact test plus the
  serial suite passed on rerun.
* Strict `cargo clippy --locked -p gateway-upstream --all-targets --all-features -- -D warnings`
  passes.
* `./scripts/check.sh fast` passes, including the complete workspace tests, the P12 serve
  envelope, source policy, crate boundaries, document links, contract references, tracked Secret
  scan, and whitespace checks.
* `cargo fmt --all`, `./scripts/check.sh docs`, `scripts/secret-scan.sh --all`, and
  `git diff --check` pass for the local change.

## Review and boundary

This is a local, no-network foundation. It does not prove proxy-node health, pool selection,
account authorization, Grok Web clearance, Provider response compatibility, staging, production,
Autoreg, or server deployment. P13-11B must add runtime composition and bounded pool observation
before any real egress probe is considered. No management OpenAPI/Prism or frontend files changed,
so no Claude Code frontend action is required for this slice.

## Formal phase gate

P13-11A is accepted as part of the aggregate P13-11 closeout:

- immutable tag: `phase-p13-egress-complete`;
- exact commit: `a716eaaa9d31c26b6d09489f3f7fdbb9b0e1ebeb`;
- formal run: [31959162202](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31959162202);
- Authorize `success` (4s), Fast `success` (6m25s), Full supply-chain `success` (1m15s), and
  Required `success` (3s).

The tag was created once and was not moved or recreated. The Gate accepts the local typed
foundation only; it does not prove a live proxy, Provider account, egress node, staging or
production deployment.
