# P13-09A stored Response foundation report

Status: `LOCAL_PASS_PENDING_PHASE_GATE`

Date: 2026-08-16

## Outcome

P13-09A adds the Provider-neutral persistence prerequisite for later Responses retrieval and
compact. Migration `0017` creates a dedicated client-owned encrypted namespace. The new store
supports exact Client Key ownership, fixed TTL, read-time expiry, bounded GC, restart recovery,
Master Key rotation compatibility, idempotent replay, collision rejection, and successful
Canonical lifecycle validation.

No public route, request decoder, Provider adapter, serving graph, production/staging deployment,
or Prism source was changed. `store`, `previous_response_id`, retrieval, deletion, and compact
remain rejected/unavailable until P13-09B/C.

## Implementation evidence

- `crates/gateway-store/migrations/0017_stored_responses.*.sql`
  - exact `(client_key_id, response_id)` primary key;
  - fixed payload version, positive key version, bounded ciphertext, lifecycle checks;
  - stable expiry index and reversible migration.
- `crates/gateway-store/src/stored_response.rs`
  - AEAD plaintext contains canonical request/events plus exact Provider/route/Credential lineage;
  - clear index contains only owner/identity/time/version/ciphertext;
  - 30-day TTL, 4096-event, 8-MiB payload, and 4096-row GC bounds;
  - exact replay idempotence and conflicting replay fail-closed;
  - read/delete treat foreign, missing, and expired identities uniformly absent;
  - Debug output redacts owner, response, Credential, model, request, and event content.
- `crates/gateway-store/src/lib.rs`
  - schema version 17 and exact migration table inventory.

## Local verification

- `cargo test --locked -p gateway-store`: PASS (`56` unit tests plus repository/backup/upgrade
  integration suites; final count recorded after the migration-specific test).
- `cargo clippy --locked -p gateway-store --all-targets --all-features -- -D warnings`: PASS.
- `cargo fmt --all -- --check`: PASS.
- `CHECK_REPORT_PATH=/tmp/cpar-p13-09a-docs.md ./scripts/check.sh docs`: PASS (`533`
  Markdown files, `107` referenced contract tests, `129` plan tasks / one `IN_PROGRESS`, tracked
  secret scan and whitespace).
- `./scripts/check-source-policy.rb`, `./scripts/check-crate-boundaries.rb`, and
  `git diff --check`: PASS. The crate-boundary allowlist records `gateway-store -> serde` because
  the AEAD payload is a typed serde envelope rather than ad-hoc JSON assembly.

Focused scenarios pass for encrypted round trip/foreign owner, conflicting replay/AAD metadata and
row-swap protection, expiry/bounded GC, reopen/key rotation, payload/lifecycle bounds, and
ciphertext corruption. A structured diff review found and corrected two durability details before
closeout: identical replay now checks durable plaintext before requesting a fresh AEAD nonce, and
clear creation/expiry instants are authenticated in AAD so TTL cannot be extended undetected. No
remaining P1/P2 issue was found in the A slice.

## Review boundary

- A is a storage foundation only; it does not claim public API compatibility.
- Exact successful-attempt lineage capture and JSON/SSE durability ordering remain P13-09B.
- Provider capability admission, exact-account continuation, and compact remain P13-09C.
- No Provider, server, production/staging, public traffic, GitHub Delivery Gate, or existing untracked
  helper was touched.
