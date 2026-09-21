# Dashboard and maintenance readiness — 2026-09-21

Base: `07d7172` plus this batch. M4 UI state acceptance progressed; no production deployment, real Provider inference or Codex with ChatGPT use.

## Defect repaired

Dashboard resource counts silently remained `…` when a count query failed. `OverviewPage` now renders the existing shared ReadStatus for resource loading/failure, labels fully retained previous counts and provides keyboard-accessible retry. Retry performs reads only. No contract or metric arithmetic changed.

EgoLite on the rebuilt real embedded gateway: browser-network blocking of `/admin/upstreams` produced `读取失败 / network_error`; after removing the block, Enter on retry restored provider/model/key counts to 1/2/2. The first attempt navigated within the old loaded document and did not exercise new code; the accepted run reloaded the rebuilt bundle. Blocking and temporary cache disabling were removed after the check. Served main.js bytes matched the built dist asset.

## Fresh bounded evidence

| Scenario | Evidence |
| --- | --- |
| Dashboard resource failure | Fixture regression preserves prior counts and exposes retry; real network failure/recovery separately confirmed above. |
| Empty request summary | Fixture returns observed zero requests and null success/latency. UI shows empty trend and `—` rather than invented zero percentages/timing; Enter opens accessible trend data. |
| Long provider name | 200-character synthetic read projection in a draft, 390px viewport: full name present and no document overflow. This is fixture evidence, not a production rename. |
| Long model identity | Existing mobile 126-character exact-ID inspector regression rerun successfully. |
| Provider operation errors | Rerun covers lost responses, dirty departure, pending writes, and explicit saved-not-applied receipts. |
| Advanced audit/backup | Real gateway/EgoLite: Enter opens resource audit detail, Escape closes it; keyboard source backup preflight returns schema 28. No backup creation/restoration performed. |
| Settings | Fresh regressions cover theme, language, and logout clearing the session. |

Runs: dashboard/settings/read-status **6 passed**; dashboard/inspectors/provider-top-level **13 passed**; final dashboard group including long provider case **3 passed**. Separate runs overlap and must not be added as unique test coverage. All successful runs used zero retries. Initial long-name test selected the fixture's empty active graph; corrected to explicitly adopt its populated draft before testing the projection, then reran the complete dashboard group.

Logs: `/tmp/prism-dashboard-readiness.log`, `/tmp/prism-resource-readiness.log`, `/tmp/prism-dashboard-final.log`. Deterministic four-file check, gateway build and 3 Rust embedded tests passed (`/tmp/prism-dashboard-build.log`, `/tmp/prism-dashboard-gateway.log`, `/tmp/prism-dashboard-embedded.log`). No full-suite rerun was required for this isolated read-status change.

## Consolidated remaining work

The concrete UI scenarios identified in the previous two readiness batches now have bounded evidence; this is not an assertion that every combinatorial state or assistive technology was tested. Read together with the integrated, keyboard and operational state reports.

Next M4 closeout is evidence reconciliation: index existing real-channel/manual authorization results by channel and implementation scope; identify any genuinely missing required result; run the applicable final release gates on the frozen delivery revision. M5 then prepares signed artifacts, rollback and authorized Oracle cutover/public checks. M4/M5 remain open until those gates are evidenced. No new unbounded “all variants” UI task is introduced.
