# P12-10G controlled live subset receipt

Status: `PASS`

Date: 2026-08-04

## Boundary

This receipt covers only the isolated P12-10G migration and native-provider parity exercise. It did
not publish a production graph, change a public listener, mutate the source account pool, restart a
production service or begin P12-10H. Full-pool parity below means that the independent source-row
count, export response count and decoded export-document count agreed; it does not claim that every
source account sent a live request.

## Revision and artifact

- Exact gateway revision: `7af49b37c6a9e4e5f8613ff8a6e0b502eeb1095d`.
- GitHub release-artifact workflow: `30851396380`.
- Both `aarch64-unknown-linux-gnu` and `x86_64-unknown-linux-gnu` build/sign jobs completed
  successfully. The ARM artifact used by the server passed the repository artifact verifier, OCI
  metadata verifier and revision check before execution.
- The source-export normalizer review fixed a same-provider display namespace mismatch and was
  committed separately as `7c68fcd`. The normalizer removes only a leading Console namespace from
  a Console audit model and rejects Build/Web namespace values instead of guessing.

## Source and admission evidence

- The existing grok2api Console route returned a successful attributed source-reference response.
- Build source rows, export count header and decoded export count all agreed at 829; exactly one
  eligible record entered the controlled batch.
- Console source rows, export count header and decoded export count all agreed at 898; exactly one
  eligible record entered its separate controlled batch.
- Web stopped before credential export with `web_expiry_unavailable`. No synthetic expiry was
  invented and no Web request was sent.

## Native CPAR evidence

The Build and Console batches independently passed all of the following through the exact signed
gateway binary:

- one source record accepted, one native account created and no rejected record;
- one direct provider attempt attributed to the imported account;
- health and quota state available for that exact binding;
- one complete terminal Canonical lifecycle;
- successful Chat Completions, OpenAI Responses and Anthropic Messages semantic projections.

The investigation leading to the final Console pass remained fail closed. Standard transport and
approximate browser headers did not pass. The final transport used an isolated Chrome-compatible
TLS/HTTP2 profile with DNS pinning, redirects disabled, bounded deadlines and the source-observed
Console request profile. The last request-class failure was traced to grok2api storing a
provider-qualified display model in its audit table while stripping that same provider namespace
before upstream dispatch. CPAR now receives only the normalized same-provider upstream model.

## Rollback and invariance

- Both isolated batches were deliberately rolled back and each removed exactly one native account.
- The isolated database ended with zero Grok accounts and SQLite `quick_check=ok`.
- The production CPAR database fingerprint remained `2a68cfedf454443c`.
- `cpa-rust-gateway.service` remained active and grok2api remained healthy.
- The isolated runtime directory, test binaries and root-only temporary helpers were removed; both
  services and the production database fingerprint were rechecked after cleanup.
- No source account was disabled, deleted, refreshed or otherwise mutated by this exercise.
- P12-10H stop/start rehearsal and representative 72-hour CPAR-only observation have not started.

## Review

The exporter unit suite, Python compilation, diff/whitespace checks and the repository fast gate
passed after the namespace fix. The live result closes P12-10G without weakening Web expiry,
credential handling, transport admission, rollback or production-isolation requirements.
