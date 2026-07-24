# P10-09 Embedded management UI and inference isolation report

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P10-09` |
| Date | `2026-07-24` |
| Branch | `codex/p10-control-plane` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Scope | Build-time embedded SPA assets, exact static routes/headers and proof of separation from public inference configuration. |

## Delivered boundary

`gateway-http-actix` now rebuilds the existing deterministic SPA before compiling its embedded
asset bytes. Its Cargo build script tracks the frozen OpenAPI/generator/build inputs plus all
current UI source inputs, invokes the no-network local SPA build, and refuses compilation if the
four required generated assets are absent. The binary embeds exact index/CSS/application/generated
client bytes, so runtime asset serving reads neither a filesystem path nor Node/npm.

`configure_embedded_management_ui` is an explicit, separate management-listener configuration. It
serves a closed `/admin-ui/` asset map only; `/admin-ui` redirects to the trailing-slash root, and
no dynamic filename/fallback route exists. Every asset returns its exact content type, `no-store`,
HTTP CSP including `frame-ancestors 'none'`, `nosniff`, `DENY` framing and no-referrer headers.
The static UI has no Management Key, CSRF, Cookie, database, configuration, Provider, Secret or
Backup/Master Key input. Protected `/admin/` API admission remains P10-02's separate boundary.

Public `configure` does not register the UI configuration. An embedding may combine management
API/UI only on its explicit management listener; P12 owns listener binding, access exposure and
deployment. P11 remains responsible for release performance thresholds and stress testing.

## Verification

| Evidence | Result |
|---|---|
| Clean `cargo clean -p gateway-http-actix && cargo check --locked -p gateway-http-actix` | PASS — Cargo rebuild invoked the local SPA embedding build before compiling the HTTP crate. |
| `cargo test --locked -p gateway-http-actix --test p10_09_embedded_management_ui -- --nocapture` | PASS — 3 tests: exact embedded bodies/content types/headers, closed asset mapping and separate UI/data-plane route configurations. |
| Local route-level probe | 2,000 `/healthz` requests: data-plane-only `7,472µs`; same test app with the UI route table `7,429µs`. This is a non-threshold local sample, not a P11 P99/RSS conclusion. |
| `cargo clippy --locked -p gateway-http-actix --all-targets -- -D warnings`, `cargo fmt --all -- --check` | PASS. |
| `npm --prefix web/admin-ui run check` | PASS — 65 generated operations and reproducible double static build. |
| Source/crate policy, `./scripts/check.sh docs`, Secret scan, whitespace | PASS — no panic/source-policy regression; 21 crate dependency boundaries and plan/doc/Secret checks passed. |

## Review

Focused review found and corrected two build-boundary issues before closeout: build errors now exit
explicitly instead of using forbidden `panic!`, and the immutable embedded-asset descriptor is
explicitly `Copy`. Review verified declared build inputs, no runtime asset lookup, exact asset
identity, redirect/content-type/header behavior, no arbitrary path route, no API/key/Secret
authority, distinct UI/data-plane registration and the P11/P12 limits. No P10-09 release-blocking
issue remains.

P10's one local Full gate, phase-level review, closeout commit and only GitHub Delivery Gate remain
before P10 can become `DONE`; no P11 task has started.
