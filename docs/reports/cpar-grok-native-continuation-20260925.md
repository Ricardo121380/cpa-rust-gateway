# Grok native continuation incident — 2026-09-25

## Evidence and scope

OMP Desktop's latest bounded run passed the first model request and approval, then failed the second
request with `502 ProviderPermanent` at 13:07:25 UTC. Its task ID is
`bb8dbe90-3f2b-49de-8edc-e84913510b84`; the gateway request suffix is `-1` under
`p1-request-dfc70894ca60504883adeea4d1729128`. The deployed binary was `f4ebf4d`.
The previous repair addressed ingress rejection, but its full gateway mock used the generic
OpenAI adapter rather than the native Grok Build response decoder.

The user separately authorized at most **two synthetic Grok Build diagnostics**, low effort,
maximum output 256 tokens each, no tool execution and no automatic retries. Both were used:

| Request | Input difference | Result |
|---|---|---|
| 1 | Synthetic reasoning history with content, no summary | HTTP 422, JSON body 131 bytes |
| 2 | Same synthetic history with `summary: []` added | HTTP 200 SSE, `response.completed`, no failed event |

These were direct requests from the existing Oracle host to the fixed Build endpoint, using an
existing unexpired Build credential in memory. They were not OMP reruns or CPAR data-plane requests;
they do not produce CPAR request/ledger acceptance evidence. No real tool ran. The first error
body's schema was not captured by the conservative sanitizer; no literal upstream missing-field
message is claimed. The single-variable comparison demonstrates the missing summary defect.
The [xAI continuation example](https://docs.x.ai/developers/model-capabilities/text/generate-text)
also includes the summary array in replayed reasoning items. The probes used synthetic item IDs,
so rewritten IDs are not demonstrated to cause this rejection.

Safe receipts are in `/private/tmp/cpar-grok-native-20260925/diagnostic-{1,2}-safe.json` locally
and `/var/tmp/cpar-grok-native-20260925/request-{1,2}-result.json` on Oracle. Persistent request
reservations prevent reusing either invocation. Credentials, raw responses, real conversation
history and generated reasoning were not stored or printed. **Budget is exhausted: 2/2.**

## Repair

- Public Responses JSON and SSE reasoning items now include `summary: []`.
- A shared Responses history encoder fills only missing/null summaries for legacy histories.
  It preserves existing summaries, content, IDs and order; stored canonical history is not mutated.
- Native Grok Build, Grok official Responses, and OpenAI-compatible builders use this encoder.
  No database migration, capability expansion, new retries or credential refresh policy is added.
- Native Build rejection logs retain the gateway request correlation, actual HTTP status,
  closed body-format classification, and exact allowlisted code/type/param values. Unknown values,
  free-form error text and response bodies never enter logs. Public error/retry classification
  stays unchanged.

## Prevention and validation

- Native adapter regression exercises JSON/SSE upstream × JSON/SSE public output × fresh/legacy
  replay: **8 combinations, 16 offline adapter requests**, including reasoning and tool results.
  It asserts required summary, exact replayed content/IDs/order and tool call correlation.
- Shared encoder tests cover missing, null, empty and nonempty summaries and source immutability.
- Error tests cover object/string/array/non-JSON envelopes and secret-like unlisted values.
- Targeted provider-grok, protocol-openai-responses and provider-openai-compatible run:
  **299 passed, 5 pre-existing ignored**, no real network inference. Formal deployment gate
  already runs these suites, so the native-chain regression is mandatory for future releases.
- Final release gate, signed artifacts, isolated rollback and production cutover passed;
  see the production evidence below.

## Remaining boundaries

This proves the synthetic continuation request shape, not the complete OMP approval/write/read
journey. OMP's separate exhausted retry budget is unchanged. No further real inference is allowed
under this diagnostic approval.

The broader audit also found native item IDs are reconstructed and native summary-only output
is not retained as a distinct summary representation. Neither is established as this incident's
cause; solving faithful opaque/native replay needs an explicit canonical metadata design and
ownership review, especially for encrypted reasoning. Do not claim arbitrary native reasoning
roundtrips are lossless. The previously documented intermittent generic mock 503 is also not
explained by this fix.

## Production evidence

Deployed `4d5ce334f595aae76d8e9230c11df57642210c3d` at **2026-09-25 15:22:47 UTC**
(23:22:47 Asia/Shanghai), following existing authorization for Oracle and its domain.

- Implementation `799723e`; router expectation update `2399644`; tracing dependency boundary
  declaration `4d5ce33`. Earlier formal runs failed on the outdated assertion and the undeclared
  tracing dependency respectively; neither candidate was deployed. Both were corrected, not waived.
- [Exact revision delivery gate](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36150102261):
  Fast, supply-chain and required gate all succeeded. Local full Rust run: 1,298 passed,
  9 existing ignored. Clippy, format, contracts, source/dependency boundaries, secret scan,
  four-file management embed and real local gateway continuation tests passed.
- [Signed ARM64 and x86_64 artifacts](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36150107483):
  independently verified signatures, manifest, SBOM and checksums.
- Production ARM64 SHA-256:
  `45786a8649720cd513eee09ae86fcab7a0ac2eadd0a2e836446a925562e6f055`.
- The signed ARM64 binary passed the real-gateway/loopback mock regression in a disconnected
  network namespace. Separately, the native Grok adapter tests run in the formal Rust suite.
- Disconnected production-copy fallback/candidate/fallback/candidate passed on schema28;
  accounts, effective grants, historical records and administrator store retained.
- Stop-to-ready **1,032 ms**. Public HTTPS health, CSP/assets, authentication boundary,
  authenticated inventory/system/requests, process executable hash and loopback binds passed.
- Existing 7 managed accounts, 2,248 event rows and 596 ledger rows retained. Four frontend asset
  hashes are unchanged. No DNS/Caddy/Autoreg edits, schema migration or historical deletion.
- Compatible fallback is signed `f4ebf4d75c111f8e0fa8e4f9d342a01bfb8da723`; keep latest databases
  and rotating credentials. Rollback reintroduces the missing-summary defect.
- Release/backup receipts: `/var/tmp/cpar-native-4d5ce334f595` on Oracle and
  `/private/tmp/cpar-native-release-20260925` locally. Deployment itself made **zero** Provider
  requests; this does not reset or exclude the **two** earlier diagnostic requests.

No further OMP task execution was initiated. Its full approval/write/read acceptance remains
separate from this repaired and deployed protocol defect.
