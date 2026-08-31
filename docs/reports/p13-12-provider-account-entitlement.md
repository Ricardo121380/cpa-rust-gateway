# P13-12 Provider/channel-scoped account entitlement report

## Status

`LOCAL_BACKEND_PASS / PRODUCTION_BACKFILL_PENDING / FRONTEND_SYNC_PENDING`

This report records the backend implementation and its current acceptance boundary. It does not
claim production installation, a durable observation for the current live account, Claude Code
contract synchronization, a formal Delivery Gate, or Pi/CC Switch end-to-end acceptance yet.

## Delivered backend behavior

- A closed core model keeps `domain`, normalized `tier`, evidence `source`, evidence `confidence`
  and `observed_at_ms` together. Missing evidence remains `null`; it never becomes `free` or
  `unknown` by default.
- Grok Build uses only `free/supergrok/heavy/unknown`. Grok Web has its own
  `basic/super/heavy/unknown` namespace. Grok Console has no frozen vocabulary and therefore stays
  `null`; neither Build nor Web values may be copied to it.
- ChatGPT/Codex uses `free/go/plus/pro5x/pro20x/unknown`; Claude uses
  `free/pro/max5x/max20x/unknown`. Ambiguous labels such as bare ChatGPT `pro`, Claude `max`, or
  fuzzy suffixes normalize to `unknown`, not a guessed paid plan.
- Generic OpenAI/Anthropic-compatible relays remain endpoint-scoped generic channels. In
  particular, an Anthropic-compatible adapter is not labelled Claude unless its composed target is
  the exact official Claude Messages endpoint.

## Evidence ownership

The only accepted source/confidence pairs are:

| Source | Confidence | Meaning |
|---|---|---|
| `provider_subscription` | `authoritative` | bounded successful response from the exact subscription endpoint |
| `signed_token` | `derived` | locally decoded JWT-shaped claim; this slice does not independently verify the signature |
| `imported_metadata` | `declared` | explicit bounded metadata supplied in an imported credential envelope |

Quota, request success, model access, Health, Circuit, priority and scheduler selection are not
entitlement evidence. The entitlement never changes those owners.

## Persistence and runtime projection

Migration `0020_grok_account_entitlements` adds one strict child table keyed by the native Grok
account ID. SQLite checks and triggers reject invalid domain/tier/source/confidence combinations,
Build/Web mismatch, and Console rows. Writes are idempotent and monotonic: an older observation or
a different observation at the same millisecond is rejected. Import-batch rollback cascades only
the corresponding child rows; credential ciphertext, account revision, Health, Quota, Circuit and
lease state are unchanged.

Ordinary ChatGPT/Codex and Claude credentials project explicit plan evidence only while composing
their exact runtime account row. Native Grok rows project the durable child row. The protected
`GET /admin/operations/provider-account-pools` response now includes nullable `entitlement`; the
authoritative OpenAPI constrains both domain/tier and source/confidence pairs.

## Controlled Grok Build synchronization

`gateway admin grok-build-entitlement-sync` is root-only and requires an import batch containing
exactly one enabled Build account. It performs at most one bodyless, DNS-pinned, redirect-denying
subscription GET. A valid response wins; otherwise only the locally held access-token tier claim
may be used as derived fallback. It never selects Web/Console, sends inference, refreshes OAuth,
retries, or prints account identity/token/response content.

## Verification completed locally

| Check | Result |
|---|---|
| `cargo test --locked --workspace --all-features` | PASS |
| `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| migration upgrade/rollback/domain checks and native persistence/cascade tests | PASS |
| Grok response/JWT mapping, conflict and malformed-shape tests | PASS |
| ChatGPT/Claude exact normalization and generic-relay isolation tests | PASS |
| management HTTP/OpenAPI and legacy `500` to safe `503` regression | PASS |
| plan, document links, contract references, source policy, crate boundaries, secret scan and working-tree whitespace | PASS |

`./scripts/check.sh docs` reaches and passes all functional documentation checks, then stops on an
already committed extra EOF blank line in Claude Code-owned
`web/prism/src/features/models/model.ts`. The current backend diff itself passes `git diff --check`.
The exact frontend repair and contract synchronization are recorded in
`docs/cross-boundary-log.md`; Codex did not modify `web/prism/**`.

## Remaining closeout work

1. Commit and push the backend branch without staging the four pre-existing untracked helpers.
2. Produce and verify a revision-bound Linux aarch64 artifact, back up the Oracle production DB and
   binary, preflight against a copied state directory, then install only if all fail-closed checks
   pass.
3. Run the exact-batch Build entitlement sync once and retain only its value-free receipt.
4. Have Claude Code synchronize the contract, fix the existing EOF whitespace issue, wire the
   account-pool UI and run its frontend gates.
5. Run the formal Delivery Gate before changing this report and P13-12 to final
   `DONE_WITH_BOUNDARY`.
