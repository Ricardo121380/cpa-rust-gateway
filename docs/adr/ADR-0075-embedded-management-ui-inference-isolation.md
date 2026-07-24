# ADR-0075 Embedded management UI with inference-route isolation

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-24` |
| Task | `P10-09` |
| Matrix / Contract | `H01-H22`、`J02`、`J08-J09`、`J11-J15`、`J18-J20`; [BC-MGMT-008](../contracts/BC-MGMT-008-embedded-management-ui-inference-isolation.md) |

## Context

P10-03 provides a deterministic static SPA build, while P10-04 through P10-08 add protected
management workflows. The SPA must become deployable without a runtime filesystem or Node
dependency. At the same time, static asset serving, UI routing and browser headers must not become
a dependency of public inference request handling.

## Decision

`gateway-http-actix` rebuilds the reviewed SPA through a Cargo build script when one of its
declared source, generator, OpenAPI or build inputs changes. The crate embeds the exact generated
HTML, CSS and JavaScript bytes; a failed or incomplete SPA build fails the Rust build rather than
shipping stale assets.

The UI has one explicit `configure_embedded_management_ui` configuration function. It serves only
the closed `/admin-ui/` asset map, has no path-derived filesystem lookup, and applies `no-store`,
HTTP-delivered `frame-ancestors 'none'` CSP, `nosniff`, `DENY` framing and no-referrer headers.
It is not included by public `configure`, which remains the data-plane route configuration. A
deployment may place UI plus protected `/admin/` APIs on a dedicated management listener; P12
owns listener binding and external exposure.

## Consequences

- The runtime needs neither a UI asset directory nor a Node/npm executable; UI bytes are part of
  the Rust artifact.
- UI source changes are a build dependency and cannot leave the binary with an older generated
  asset set.
- `/admin-ui/` is static-only and carries no Management Key, CSRF token, backup material,
  database/configuration/Provider handle or browser persistence capability.
- The public inference configuration has no UI route. P10 supplies structural and local
  route-level evidence; P11 retains release performance thresholds and stress testing.

## Alternatives considered

- Runtime static-file serving: rejected because deployment filesystem state would become a
  management dependency and create path/symlink/cache concerns.
- Registering UI routes inside the public `configure`: rejected because it weakens the explicit
  data-plane/control-plane configuration boundary.
- Embedding a generic directory router: rejected because a closed, named asset map is simpler to
  audit and cannot turn an HTTP path into a filesystem lookup.

## Validation and rollback

The P10-09 integration tests compare each response body with the fresh SPA build, reject an
unknown asset, require hardened headers, verify the two configurations do not share routes, and
emit a local route-level comparison without treating it as a P11 threshold. A clean
`gateway-http-actix` build reruns the SPA build. Rollback removes the build script and UI module;
the independent SPA source and public inference handlers remain unchanged.
