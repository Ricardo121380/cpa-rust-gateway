# CR-P13-11-GENERIC-ENDPOINT-EGRESS-001 · Generic compatible endpoint egress boundary

| 项目 | 值 |
|---|---|
| 状态 | **Approved（用户于 2026-08-16 明确澄清）** |
| 适用范围 | P13-11 CPAR egress/proxy-pool implementation |
| 当前切片 | `P13-11A` generic compatible-endpoint profile |
| 不包含 | Autoreg registration/SSO/refresh、真实 Provider probe、生产/staging/server、管理 UI |

## 1. User clarification

Krill is not a special protocol or a privileged global channel. It is one configured instance of a
generic compatible endpoint that supplies a `base_url + api_key` (or an equivalent imported
credential envelope). CPA JSON, Sub2API JSON, direct OAuth, plain API-key, and a custom relay must
be supported through the same endpoint/credential/egress abstraction.

## 2. Required CPAR behavior

* Keep exact `Upstream/Endpoint/Credential` ownership and the selected `EgressPolicy` together.
* Let the endpoint's declared wire protocol and Provider adapter decide request/response behavior;
  the credential source label must not select a hidden proxy or fallback branch.
* Keep endpoint, credential, account-pool Health/Quota/Circuit, and egress-node failures separate.
* Permit direct, fixed-proxy, and Provider-scoped proxy-pool profiles, with explicit account/egress
  stickiness and bounded pre-submit retry only.
* Never fall through to another Provider, channel, credential, or egress pool implicitly.

## 3. Execution order

1. P13-11A freezes and tests the local typed profile and reuses existing `EndpointUrl`/
   `EgressPolicy` static admission.
2. P13-11B composes the profile with active Config Version, existing credential pools, transport
   profiles, and runtime Health/Quota without opening a second scheduler or Store path.
3. A later Provider-specific slice may add sticky proxy-node observations, bounded probes, or Web
   clearance only when its own capability and external evidence are available.

The change does not authorize a real egress request. It also does not move Autoreg ownership into
CPAR or change the existing public/management API surface.
