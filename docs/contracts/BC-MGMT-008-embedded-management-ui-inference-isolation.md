# BC-MGMT-008 Embedded management UI and inference-route isolation

| Field | Value |
|---|---|
| Contract | `BC-MGMT-008` |
| Task | `P10-09` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Domain | Build-time management SPA embedding, static-response hardening and public data-plane isolation. |

## Entry and preconditions

The tracked P10-03 SPA source, generated client, OpenAPI input, lockfile and build script are the
only declared UI-build inputs. A Rust build that embeds the UI must first complete that deterministic
local build; it may not read a runtime asset path, call a remote resource, or start Node/npm after
the binary is produced.

`configure_embedded_management_ui` is the only UI route registration point. It is intended for an
embedding's dedicated management listener. The public `gateway_http_actix::configure` data-plane
function must not register this configuration.

## Invariants

1. The embedded set is closed: `/admin-ui` redirects to `/admin-ui/`; only the index, stylesheet,
   application module and generated-client module are served. An unknown UI path returns the
   framework's absent-route result and never reaches a filesystem lookup.
2. Every static asset response has its exact reviewed bytes/content type, `Cache-Control: no-store`,
   HTTP CSP containing `frame-ancestors 'none'`, `X-Content-Type-Options: nosniff`,
   `X-Frame-Options: DENY` and `Referrer-Policy: no-referrer`.
3. UI serving reads no Management Key, CSRF token, Cookie, Secret, Backup/Master Key, database,
   configuration Snapshot, Provider state, network target or file path. Protected `/admin/` API
   admission remains governed by BC-MGMT-003.
4. Public health/inference configuration contains no UI route. Data-plane requests cannot invoke
   the embedded-asset handler; a management-only configuration contains no public data-plane route.
5. P10's local route-level comparison is structural evidence only. No P11 p99/RSS/throughput
   threshold, production listener bind, deployment, Caddy/Cloudflare rule or external request is
   implied by this contract.

## Corresponding evidence

- The Cargo build script lists its UI inputs, rebuilds the static output and verifies every asset
  before `include_bytes!` compilation.
- `p10_09_embedded_management_ui` compares each embedded response to the deterministic build,
  verifies exact headers and closed route mapping, confirms separate configuration ownership, and
  logs a repeatable local route-level comparison.
- P10-09 focused review checks build/runtime boundaries, content/headers, route isolation, lack of
  credential/filesystem expansion and P11/P12 exclusions.
