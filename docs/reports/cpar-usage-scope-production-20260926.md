# Scoped usage query production repair — 2026-09-26

Production released signed `3a164322c5731747cceb268e26249349953281d6` at **16:24:44
Asia/Shanghai** (08:24:44 UTC). Schema28; stop-to-ready1,018ms. Process executable SHA-256
`979dda0beadc98e8eec3037ab2d864a04cb6dd30c2053611942df987762ba59f`.

The [logged-in EgoLite check](cpar-postrelease-browser-20260926.md) found that four historical
ambiguous request lineages were included in every Token usage query despite time/dimension
filters. The scoped fix applies known exclusions before validation, keeps unknown-scope and
in-scope invalid data fail-closed, and preserves cursor/snapshot completeness.

## Production behavior verified

| Range | HTTP result | Usage requests | Independently counted ledger rows |
|---|---:|---:|---:|
|24 hours|200|28|28|
|7 days|200|30|30|
|30 days|200|491|491|
|All history|500, retained validation rejection|Unknown|699 validated ledger rows remain available separately|

Thirty-day results were read across three pages (limit2 groups), with one consistent observation
watermark. No aggregate is claimed for the ambiguous all-history scope. The existing opaque500
message remains a UX limitation; a follow-up can expose a safe scoped lineage error without
misrepresenting excluded history as complete usage.

Original699 ledger rows and2,356 events are retained;11 unresolved `invalid_lineage` events
remain. Existing7 managed accounts, administrator store, active configuration and effective model
permissions retained. Four frontend asset hashes are unchanged. Public CSP, protected management
boundary, process hash and authenticated production API readback passed.

## Release evidence

- [Formal gate36228791440](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36228791440):
  fast and supply-chain passed,1,309 Rust tests passed,0 failed,9 existing ignored.
- [Signed artifacts36228794418](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36228794418):
  ARM64 and x86_64 succeeded; manifest, SBOM and Sigstore identity independently verified.
- Exact signed ARM64 binary passed isolated loopback Agent/request-chain regression without
  Provider traffic. Fresh production-copy fallback→candidate→fallback→candidate demonstrated
  recent queries500→200→500→200 while all-history rejection,699 ledger rows,11 failures,
  permissions and administrator data remained stable.
- [Formal checks](evidence/cpar-usage-scope-release-20260926/formal-checks.json),
  [artifact regression](evidence/cpar-usage-scope-release-20260926/artifact-gate.json),
  [rollback rehearsal](evidence/cpar-usage-scope-release-20260926/rehearsal.json),
  [deployment](evidence/cpar-usage-scope-release-20260926/production-receipt.json),
  [postcheck](evidence/cpar-usage-scope-release-20260926/postcheck.json).

Fallback is signed `b39ec3238dbd0ae5abe67ddf0c3d799188b423b9`, same schema28. Keep latest state
and credentials; binary-only rollback restores the previous scoped-query error, not data loss.
Private backups/scripts: `/var/tmp/cpar-usage-scope-3a164322c573` on Oracle.

## Browser and remaining limits

The pre-repair logged-in browser check genuinely verified native Build/Console panels, account
detail layout/focus, ledger pagination and needs-repair status on the unchanged frontend.
Build's three observed bindings include only one authenticated/schedulable account and two expired
ones; two Console accounts lack identity metadata. No refresh/reauthorization was performed.

After service restart, the old browser session correctly returned to `#/unlock` on the next
management read, removed the authenticated view and left the password empty. EgoLite space1/p1
was handed back for a normal login. The user then confirmed login; the16:38–16:40 EgoLite recheck
visibly verified28/30/491 usage-bearing requests, model grouping446+45=491, six token confidence
fields and desktop/mobile layouts. Ledger699 and pending11 remained visible. All-history still
shows the generic error and remains an open limitation. [Browser evidence](evidence/cpar-postrelease-browser-20260926/usage-recheck.json)
is separate from the earlier API/copy-rehearsal evidence. The authenticated24h usage page was kept
open for user inspection; browser control was released.

No real inference, new OAuth, manual catalog refresh, DNS/Caddy/Autoreg change, account deletion
or historical data rewrite. CPAR2/2 and OMP3/3 tasks,5/5 retry budgets remain unchanged.
