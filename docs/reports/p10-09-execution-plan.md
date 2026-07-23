# P10-09 Embedded management UI and inference-path isolation plan

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P10-09` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Date | `2026-07-24` |
| Scope | Deterministic management SPA build-time embedding, dedicated static UI configuration, response hardening, and proof that UI serving cannot enter the public inference route configuration. |
| Inputs | [P10-03 SPA contract](../contracts/BC-MGMT-004-management-spa-generated-client.md), P10-02 admission, P10-04 through P10-08 protected operations, and P10 G10 isolation requirement. |

## Fixed design

The existing deterministic `web/admin-ui` build remains the sole source of UI assets. A
`gateway-http-actix` build script rebuilds it only when its declared UI/generator inputs change;
the crate embeds the exact generated HTML, CSS and JavaScript asset bytes. This introduces no
runtime filesystem, Node, npm, network, database, Secret, Provider or configuration dependency.

`configure_embedded_management_ui` is a separate opt-in Actix configuration for a dedicated
management listener. It serves only `/admin-ui/` plus a fixed, closed asset list and does not
accept a path-derived filename. The existing public `configure` remains the inference route
configuration and must not register the UI. A deployment can compose the management API and UI on
its explicitly configured management listener; P12 owns external listener binding and exposure.

Every embedded response has deterministic content type and `no-store`, CSP with HTTP-delivered
`frame-ancestors 'none'`, `nosniff`, `DENY` framing and no-referrer headers. The static page has
no credential or session state; `/admin/` API admission remains P10-02's separate authority.

## Exclusions

- No new OpenAPI operation, provider request, backing-store read/write, credentials, backup
  creation, listener bind, Caddy/Cloudflare setting, production deployment, or P11 benchmark
  threshold.
- No UI route is added to public `/v1/*` inference configuration. P12 owns process/listener
  binding, and P11 owns Criterion thresholds, stress and release performance work.

## Verification sequence

1. Build-script tests prove declared UI inputs generate the exact embedded assets and missing
   build prerequisites fail before a binary with stale assets can be produced.
2. Actix integration tests prove the closed `/admin-ui/` route/asset map, redirect, body identity,
   hardened response headers and no file/path fallback.
3. Isolation tests run the management UI and data-plane configurations independently, prove
   inference routes never invoke a UI asset handler, and record a repeatable local route-level
   timing comparison as evidence rather than a P11 performance threshold.
4. Review checks build input boundaries, response headers, asset body/content-type mapping,
   static-only configuration ownership, no API/Secret expansion and P11/P12 exclusions.
