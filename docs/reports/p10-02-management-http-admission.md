# P10-02 Management HTTP admission boundary

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P10-02` |
| Date | `2026-07-23` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` — implementation, focused tests, independent review and the required local Full gate passed; P10's one Delivery Gate remains at G10. |
| Scope / Task Card | Separate management authentication, actual-peer network admission, audit identity and browser CSRF/CORS boundary only; no listener bind, resource CRUD, UI, OAuth, Provider request, database mutation, backup/restore or proxy configuration. |
| Matrix / references | `H01-H02`, `H21`, `J02`, `J08-J09`; [ADR-0070](../adr/ADR-0070-management-http-admission-boundary.md); [BC-MGMT-003](../contracts/BC-MGMT-003-management-http-admission.md); [P10-01 contract](p10-01-management-openapi.md) |

## Delivered behavior

`gateway-http-actix::management_security` supplies a future `/admin/` Scope middleware. It requires
an explicitly mounted `ManagementHttpState`, then admits a request only when all of the following
are true:

1. Its actual Actix peer is loopback by default; the opt-in private policy adds only RFC1918 IPv4
   and IPv6 ULA. Forwarding headers are ignored, and missing/public/link-local/CGNAT peers fail.
2. It supplies exactly one `X-Management-Key` that constant-time matches a configured,
   zeroizing `mgmt_` secret. Neither `Authorization` nor `X-Api-Key` can fall back to a Client Key
   or become administrator authentication.
3. It has no browser Origin by default. An explicit same-origin UI policy permits only one
   canonical HTTP(S) origin, requires a separate zeroizing `csrf_` token for unsafe methods, and
   denies every Origin-bearing `OPTIONS` preflight.

Successful handlers receive only the fixed safe `ManagementRequestPrincipal` audit actor
`management-key`; P10-04+ resource mutations must pass it to their existing transactional audit
operation. P10-02 creates no audit row, since it performs no management resource action.

Every denial shares one `404`, `Cache-Control: no-store`, bounded JSON envelope. There is no
`WWW-Authenticate`, CORS grant, error reason, key material, client-key fallback, forwarded-IP
trust or Provider/SQLite side effect. P10-02 itself registers no resource handler; P10-04+ must
supply frozen OpenAPI routes through `configure_management`.

## Targeted verification and review

| Command / review | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --locked -p gateway-http-actix --test p10_02_management_security` | PASS; 4 adversarial in-process tests cover missing/wrong/duplicate key headers, Client-Key separation, missing state, peer policy/forwarded-header non-authority, cross-origin/default denial, exact same-origin mutation CSRF, no-CORS response, actor attachment and redaction. |
| `cargo clippy --locked -p gateway-http-actix --test p10_02_management_security -- -D warnings` | PASS |
| `./scripts/check-doc-links.rb`, `git diff --check` | PASS |
| `./scripts/check.sh full` | PASS; workspace fast/full checks, supply-chain checks, crate boundaries, documentation checks and tracked Secret scan passed. |

Focused review verifies that the Management Key and CSRF token have no string accessor or Debug
value; only one management header is read; `peer_addr` rather than a forwarded header decides
admission; no normal response emits a CORS allow header; preflight cannot be accepted; failure
does not reveal whether the source, key, origin or token was wrong; and the public data-plane
router is untouched.

## Rollback and next task

Rollback removes the security Scope/helper and its documentation/tests. It does not alter the
OpenAPI contract, SQLite, Config Versions, active Snapshot, public API, Client Keys, Credentials,
runtime state, server bind configuration or external traffic. P10-03 is next and may build the
static SPA/API client while preserving P10-02's guarded-scope integration point.
