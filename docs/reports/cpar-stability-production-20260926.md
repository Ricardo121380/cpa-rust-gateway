# CPAR stability production release — 2026-09-26

Released to the existing Oracle Singapore service and domain at **2026-09-26 15:26:07
Asia/Shanghai** (07:26:07 UTC). Production serves signed
`b39ec3238dbd0ae5abe67ddf0c3d799188b423b9`, schema28. Stop-to-ready was **1,046 ms**.
The exact running executable SHA-256 is
`631ff35b4192cb3ff875f0ba6158ea8c3529cea14380a4cb522bd4794afb2a8c`.

Entry: [Prism administration](https://cpar.142857142.xyz/admin-ui/).

## Delivered behavior

- `297401f`: native Grok output IDs, reasoning summary/content parts and JSON/SSE history
  fidelity. [Implementation and limits](cpar-native-output-fidelity-20260926.md).
- `86ea204`: provider details list native Grok accounts using the native inventory; runtime
  counts use the observed provider/endpoint pool, not ordinary credential bindings. Existing
  detail/reauthorization/enrollment flows remain. Legacy Usage IDs are accepted with index and
  payload consistency validation. [Original findings](prism-production-browser-20260926.md).
- `0bc7cae`: Agent fixture readiness waits for actual catalog initialization; failures retain
  correctly scoped runtime evidence. [Failed gate and correction](cpar-release-gate-readiness-20260926.md).
- `b39ec32`: historical requests with multiple distinct Usage records cannot be assigned to
  one Attempt. [Production-copy discovery and preserved ambiguity](cpar-legacy-billing-lineage-20260926.md).

## Production readback

| Check | Observed result |
|---|---|
| Native Build detail data | 3 managed native accounts, 3 observed runtime accounts, Responses |
| Native Console detail data | 2 managed native accounts, 2 observed runtime accounts, Responses |
| Existing managed inventory | All 7 accounts retained |
| Historical event log | All 2,356 original rows retained |
| Historical ledger | All 623 original rows unchanged; 76 new rows, total 699 |
| Legacy billing recovery | 76 of 87 source IDs materialized exactly once; all `unpriced`, amount null |
| Remaining history | 11 `invalid_lineage` rows across 4 reused request IDs; original evidence retained |
| Processing | Checkpoint equals source ordinal 2,356; `needs_repair` correctly reflects 11 unresolved rows |
| Authorization/configuration | Existing administrator store, active configuration and effective model permissions retained |
| Public deployment | Four embedded asset hashes match candidate; CSP and management authentication boundary verified |

The remaining 11 rows are not silently discarded or assigned zero cost. Six belong to three
request IDs with two different Usage observations but one successful Attempt; five belong to
one request ID with one failed Attempt. Reliable reconciliation needs the original per-call
correlation evidence. This release does **not** claim all 87 records have been billed.

The authenticated host-local API readback is [saved here](evidence/cpar-stability-release-20260926/postcheck.json).
It is not a post-release logged-in browser check. EgoLite's existing space1/p1 was returned to
the user at the production login page; a normal administrator login is pending. No password
reset, session bypass or retry with the old initial password was attempted. Earlier 48-state
browser evidence belongs to the prior production revision; the new native panel also has local
component and synthetic real-gateway evidence described in the original findings report.

## Release gates

- [Final formal delivery run](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36226178509):
  Fast, full supply-chain and Required delivery gate passed on `b39ec32`; 1,308 Rust tests
  passed, zero failed, 9 pre-existing ignored. [Check receipt](evidence/cpar-stability-release-20260926/formal-checks.json).
- [Signed ARM64 and x86_64 build](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36226180703):
  both targets passed; local independent manifest/SBOM and Sigstore identity verification passed.
  [ARM64 verification](evidence/cpar-stability-release-20260926/verified-artifact.json),
  [x86_64 verification](evidence/cpar-stability-release-20260926/verified-x64.json).
- Exact signed ARM64 binary executed on Oracle in a disconnected network namespace under the
  dedicated service user with TLS loopback mocks: 12 successful Agent rounds/ledger records,
  malformed ingress, failure, truncation, cancellation and paginated catalog/grant checks passed.
  [Artifact receipt](evidence/cpar-stability-release-20260926/artifact-gate.json).
- Fresh production-copy old → new → old → new rehearsal passed. Ledger progression
  **623 → 699 → 699 → 699**, prior rows and permissions retained, administrator copy unchanged.
  [Rehearsal](evidence/cpar-stability-release-20260926/rehearsal.json).
- Atomic binary-only cutover and public/authenticated readback passed.
  [Deployment receipt](evidence/cpar-stability-release-20260926/production-receipt.json).

Negative gates were retained, not waived: `86ea204` failed a mock Agent round; `0bc7cae`
passed CI/signing but failed production-copy recovery, revealing ambiguous historical lineage.
Neither candidate was deployed. The corrected candidate completed the whole path above.
GitHub also emits a non-blocking Node20 action-runtime deprecation annotation; this release
does not change workflow action pins solely to silence that notice.

## Rollback and scope

Immediate signed fallback is `4d5ce334f595aae76d8e9230c11df57642210c3d`, also schema28.
Switch binary only and retain the latest databases, restored ledger rows, administrator and
rotating credentials. Never restore an old database to erase new writes. The rehearsal verified
this latest-state roundtrip. New optional native item lifecycle payloads need an aware binary
for full replay; an older binary may require a new conversation. Do not claim backward replay.

Private stage, backups and scripts: `/var/tmp/cpar-stability-b39ec3238dbd` on Oracle;
local receipts: `/private/tmp/cpar-stability-final-20260926`. No production configuration
publication, account deletion, DNS/Caddy/Autoreg changes, new OAuth flow or manual external
Provider request occurred. Existing service workers resume under their existing configuration.

No new real inference was performed. CPAR diagnostics remain 2/2 consumed; OMP tasks 3/3 and
retries 5/5 remain exhausted. The existing four-round live acceptance is earlier evidence, not
a live test of this new revision. Historical unexplained 503 and broader channel authorization
claims remain bounded by their original evidence. Next narrow checks: post-release logged-in
EgoLite provider/usage views and evidence-based reconciliation of the 11 historical rows.
