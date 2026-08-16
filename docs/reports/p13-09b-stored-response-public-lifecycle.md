# P13-09B stored Response public lifecycle report

Status: `DONE_WITH_BOUNDARY`

Date: 2026-08-16

## Outcome

P13-09B enables gateway-owned opt-in storage on `POST /v1/responses` and exact Client-Key-owned
`GET/DELETE /v1/responses/{id}`. The routed executor records the actual successful Config/
Provider/Channel/Route/Candidate/Credential revision directly from the selected candidate and live
lease. JSON and SSE share one Canonical payload and withhold their successful terminal until the
encrypted P13-09A transaction commits.

`previous_response_id` and `/v1/responses/compact` remain rejected for P13-09C. This slice does not
add Provider calls, touch production/staging/server state, run a real probe, change management
OpenAPI/Prism, or start P13-10.

## Implementation evidence

- `crates/protocol-openai-responses/src/lib.rs`
  - accepts only boolean `store`;
  - treats it as a gateway control rather than a Canonical raw extension;
  - normalizes opt-in native forwarding to `store:false`.
- `crates/gateway-router/src/execution_lineage.rs` and `crates/gateway-router/src/lib.rs`
  - request-local single-assignment, value-free successful-attempt lineage recorder;
  - explicit executor capability boundary.
- `apps/gateway/src/runtime.rs`
  - records exact final selected candidate and Credential lease revision;
  - opens the encrypted stored-response repository from the same database and external SecretStore.
- `crates/gateway-http-actix/src/lib.rs`
  - opt-in create, exact owner GET/DELETE, uniform safe not-found, and `no-store` cache boundary;
  - complete Canonical validation and durable write before downstream successful terminal;
  - shared JSON/SSE payload and failure/partial/StreamError exclusion.

## Local verification

- `cargo test --locked -p protocol-openai-responses -p gateway-router -p gateway-store -p
  gateway-http-actix`: PASS. Core unit counts were protocol `25`, router `135`, store `56`, and HTTP
  `58`; all applicable package integration suites passed, with only their explicitly authorized
  real/property/soak tests remaining ignored.
- `cargo test --locked -p gateway`: PASS (`106` runtime/application unit tests plus the component
  smoke test).
- `cargo clippy --locked -p protocol-openai-responses -p gateway-router -p gateway-store -p
  gateway-http-actix -p gateway --all-targets --all-features -- -D warnings`: PASS.
- `cargo fmt --all -- --check`, `./scripts/check-source-policy.rb`,
  `./scripts/check-crate-boundaries.rb`, `git diff --check`, tracked/staged Secret scans, and the
  P13-09B documentation Gate: PASS.
- Focused regression proves boolean decode and upstream `store:false`; exact selected
  Provider/Channel/Route/Credential revision; JSON create/GET/delete; SSE durability before
  completed; StreamError no-store; 4096-event incremental capture failure; foreign owner GET and
  DELETE isolation; uniform missing/deleted behavior; auth/no-store headers; and missing-lineage
  rejection before executor start.

## Review

A structured code/diff review found and repaired two issues before closeout:

1. the first HTTP capture accumulated events until final store validation, so a long successful
   stream could temporarily exceed P13-09A's durable limits; capture now enforces the same event and
   serialized-byte limits incrementally and fails before forwarding a completed terminal;
2. an attempted runtime-loopback lineage test conflicted with the production composition's
   intentional HTTPS-only egress admission; the security boundary was retained, and exact
   candidate/live-lease mapping is instead tested without network while HTTP durability uses the
   deterministic executor.

The full HTTP suite also exposed a P13-08 baseline regression where the read-only billing catalog
list used a write/`If-Match` context. That unrelated regression was restored and committed
separately as `bf6c9fd`; its P13-05C HTTP suite passes. No remaining P1/P2 correctness, ownership,
resource-bound, or secret-exposure issue was found in P13-09B.

## Boundary

- P13-09A remains the AEAD/TTL/GC/restart authority.
- P13-09C remains responsible for exact-account `previous_response_id` and capability-specific
  compact semantics.
- No existing untracked helper was modified or staged.
- No Provider, deployment, production/staging/server, management OpenAPI/Prism, or expensive
  GitHub Delivery Gate action was performed.

## Aggregate formal acceptance

P13-09B was included without broadening its public lifecycle scope in annotated tag
`phase-p13-responses-complete` at exact commit
`d419c4678bd2ff563046849cef800c1985d48688`. GitHub Actions run
[31922870604](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31922870604)
passed Authorize, Fast, Full supply-chain and Required in `2s` / `6m10s` / `59s` / `3s`.
Together with the aggregate local Full `43/43`, this closes P13-09B as `DONE_WITH_BOUNDARY`; it
does not claim a real Provider or production stored-response execution.
