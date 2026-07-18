# P1-07 Actix Responses handler report

| Field | Value |
|---|---|
| Plan | `v1.0` |
| Task | `P1-07` |
| Date | `2026-07-18` |
| Branch | `codex/p1-07-actix-responses-handler` |
| Rust | `1.97.1` |
| Result | Local PASS; independent review and GitHub CI pending |

## Delivered scope

- Added Actix `GET /healthz` and raw-body `POST /v1/responses` handlers.
- Kept duplicate JSON-name protection intact by feeding complete raw UTF-8 request bytes directly
  to the P1-05 `decode_request` codec rather than using `web::Json`.
- Added a router-owned execution facade and deterministic-Mock adapter so HTTP does not depend on
  or expose P1-06 Provider types.
- Composed the source with P1-04 explicit bounded canonical transport, cancellation-aware Tokio
  producer ownership, and downstream-only FirstSemanticEvent controls.
- Added non-streaming Responses JSON and typed SSE output, including pre-header JSON envelope
  errors, post-header safe failure conversion, quiet cancellation, and response terminality rules.
- Added [BC-HTTP-001](../contracts/BC-HTTP-001-actix-responses-handler.md) and updated the exact
  crate-boundary policy for HTTP-local Tokio/future primitives.

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-http-actix` | PASS; 12 in-process HTTP/body tests plus doc tests |
| `cargo test --locked -p gateway-router` | PASS |
| `cargo clippy --locked -p gateway-router -p gateway-http-actix --all-targets --all-features -- -D warnings` | PASS |
| `ruby scripts/check-crate-boundaries.rb` | PASS; HTTP retains no direct Provider dependency |
| `cargo fmt --all -- --check` | PASS |
| `./scripts/check.sh fast` | PASS; format, Clippy, full workspace tests, source/secret/boundary/doc checks |
| `./scripts/check.sh full` | PASS; fast gate plus dependency policy and RustSec audit |
| `git diff --check` | PASS |

## Independent review

- Verify the router facade keeps `gateway-provider` types out of the HTTP public surface.
- Verify the successful Actix body handoff, rather than queueing/dequeueing/encoding, is the sole
  FirstSemanticEvent commit point for both JSON and SSE.
- Verify post-header EOF, out-of-band errors, and encoder errors yield one safe failure without a
  fabricated completion, while cancellation remains quiet.

Review passed. The HTTP crate's direct dependency tree contains no `gateway-provider`; the router
facade is the sole Provider boundary. The review also exercised the complete local fast and full
gates after inspecting the body-poll FSE commit point, cancellation/drop behavior, and terminal
failure conversion. Hosted GitHub CI remains the final task-completion condition.

## Limits and next task

P1-07 is intentionally unauthenticated and uses only the deterministic Mock path. It does not
implement client-key auth, persistent configuration, real Provider selection, retries, management
surfaces, P1-08, P1-09, P2, or server deployment. The task remains `IN_REVIEW` until independent
review and all required gates complete.
