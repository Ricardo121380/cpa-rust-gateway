# BC-SEC-005: Generic compatible-endpoint egress profile

Status: `DONE_WITH_BOUNDARY`

## Scope

This contract covers the local P13-11A typed foundation for any configured compatible endpoint
whose operator supplies a `base_url + api_key` or an equivalent imported credential envelope. CPA
JSON, Sub2API JSON, direct OAuth, and a relay commonly called Krill are source/instance labels, not
different global providers.

## Required invariants

* One profile binds exactly one upstream/provider instance, endpoint/channel, credential, protocol,
  and version-scoped egress policy.
* Endpoint-owner and credential-owner upstream IDs must both equal the profile upstream ID.
* Egress target is closed to direct, fixed proxy, or named proxy pool. There is no implicit global
  fallback or cross-Provider borrowing.
* Failure scope is endpoint, exact credential, or selected egress node. Runtime Health/Quota and
  credential status remain separate stores.
* Stickiness is explicit (`none`, `credential`, or `credential_and_egress`) and direct transport
  cannot claim an egress-node assignment.
* Retry is none or a total-attempt budget from one through three, and applies only before upstream
  submission. A post-submit replay is not representable.
* Labels and opaque identities are bounded to 128 bytes, reject surrounding whitespace/control
  characters, and validation errors contain no supplied URL, key, token, cookie, header, or
  request body.
* Static endpoint checks reuse `EndpointUrl` and the selected `EgressPolicy`; they do not resolve
  DNS or open a socket. The later transport must still perform per-attempt DNS/address admission.

## Explicit non-goals

P13-11A does not persist proxy pools, probe nodes, sticky assignments, clearance/session cookies,
Provider account refresh, Autoreg registration, management UI, OpenAPI/Prism routes, or real
Provider traffic. It also does not claim that a profile makes Grok Web, Console, Build, Codex,
Anthropic, or a custom relay usable; those capabilities remain explicit per adapter and account.

## Security and isolation

The profile contains only opaque IDs and non-secret labels. `Debug` output contains no endpoint URL
or credential material. A later runtime adapter must use the same Config Version, exact credential
pool, Health/Quota registry, and egress admission used by serving. A failed endpoint, credential,
or egress node may not cause fallback to another Provider or another credential format.

## Verification target

The first local slice must prove relay-name neutrality, CPA/Sub2API metadata neutrality, exact
binding ownership, URL/SSRF policy reuse, bounded retry/stickiness/failure-scope validation, and
secret-free debug/error behavior without any Provider or network call.

## Formal acceptance

This contract is accepted as P13-11A within the aggregate formal Gate:
`phase-p13-egress-complete` at `a716eaaa9d31c26b6d09489f3f7fdbb9b0e1ebeb`, run
[31959162202](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31959162202). All four
jobs passed. The contract does not claim live proxy, Provider, staging or production evidence.
