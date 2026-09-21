# Prism integrated acceptance — 2026-09-21

Status: **M4 in progress; not a production release or whole-plan completion.**
Real embedded gateway implementation under test: `b02329a`; the final rebuilt working tree additionally fixes the usage-page null-group label and account search/form race. This batch reconciles regression tests with the accepted workspace/navigation lifecycle. No Codex with ChatGPT or real Provider inference was used.

## Real embedded gateway

A fresh disposable state directory and owned loopback TLS Provider were started by `scripts/acceptance/prism-local-gateway.py`. EgoLite exercised the embedded `/admin-ui/`, not the Vite fixture app.

- First administrator login and required password change completed; credentials remain in mode-0600 local files, never in this report.
- `prism-request-chain.py`: three catalog models across two pages; refresh did not widen the single served model or existing key permissions. Five external requests reached distinct terminal outcomes: two successful, two failed, one cancelled; five attempts. Successful requests materialized into billing. Stream truncation did not count as success.
- UI selected `Model-Second`, connected and applied it; key creation selected all two open models, then explicitly narrowed to `Model-Second`. The reveal-once secret was saved directly to a protected file and removed from the DOM via the completion action.
- `prism-ui-model-key.py`: new key scope verified, existing key unchanged, cross-model call denied, mock inference recorded, missing price retained as unpriced with unknown cost.
- Kimi Coding onboarding offered authorization login and OAuth import without unrelated provider/endpoint selection. No official authorization was started in this batch.

Sanitized machine-readable evidence: [local receipts](prism-integration-local-20260921.json). Duration observations are synthetic correctness evidence, not production performance claims.

## Browser scope

EgoLite/Chromium, 1440×900, 1280×720 and 390×844: 14 routes including retained advanced entries, in light and dark themes (84 structural checks). No document horizontal overflow or main alert at the recorded observation points. This matrix is structural; it does not certify every operation, loading state or visual detail.

Provider edit screenshots inspected at desktop/light and phone/dark; the settled modal has legible opaque content and visible footer actions. Explicit reduced-motion and reduced-transparency emulation confirmed matching media queries, no panel animation and no scrim backdrop filter. Preference evidence is for this form, not inferred from all navigation checks. Kimi onboarding screenshot also inspected.

Local artifacts: `/tmp/prism-integration-20260921/`. First transition-frame screenshot was not used as settled visual evidence.

## Regression status

- Frontend unit suite: 48 files, 390 tests passed in this batch.
- TypeScript passed.
- Full Chromium suite and targeted reconciliation completed. Initial failures include stale internal-ID labels, pre-adoption draft selection, obsolete direct publication buttons, obsolete fixed form width, and collapsed advanced telemetry/maintenance sections. Security, exact identity targeting, mutation counts and terminal-state assertions are retained.
- Initial full run: 248 passed / 38 failed. Final frozen implementation run: 286 passed / 1 failed (6.4 minutes, 2 workers, zero retries). The sole failure was the new clock-controlled regression freezing an asynchronously scheduled query notification. Its wait now advances the fake clock until the channel selector renders; the complete account-presentation group then passed 8/8 (21.9 seconds, one worker, zero retries). These are separate runs, not a claim of one 287/287 all-green invocation. Logs: `/tmp/prism-integration-full-final.log`, `/tmp/prism-integration-account-final.log`.
- One actual rendering regression was found: the `(无访问组)` aggregation sentinel was incorrectly resolved as a resource name. The usage renderer now preserves this sentinel; the original failing browser case and 29 usage-model units pass.
- A second real regression was fixed: pending search navigation could interrupt a newly opened authorization form. The operation owner now suspends the search timer until close; the new regression verifies no discard prompt, preserved dirty material, and resumed search after an explicit discard. Three viewport import flows pass.
- Final deterministic four-file/SPA checks and 3 embedded Rust tests passed; the owned local gateway was rebuilt and restarted. Its Kimi import form/explicit discard flow was checked in EgoLite; no official authorization was submitted.
- All initial failures have passing successor coverage. Production signing/deployment and remaining release readiness review are not completed by these local results.

## Performance and remaining work

Same machine, 512 synthetic prices, one excluded warmup then five measured samples per revision. Before `a2959ef`, after `b02329a` plus the two production fixes (hashes recorded). Median startup: 156.5 → 163.1 ms; editor: 846.8 → 834.2 ms; filter: 822.1 → 873.3 ms. Filter delta is 51.2 ms / 6.2%, so it does not exceed both required thresholds. Editor sample five was 1869.1 ms; the outlier is retained, not discarded. Both sides mounted 50 rows and made one editor / five filter API invocations. These are fixture/development measurements, not network or production SLA results. [Raw samples and revisions](prism-integration-performance-20260921.json).

M4 remains open for the final workspace-by-workspace release readiness review, including explicit gaps in keyboard, long/error states and real-channel/manual authorization evidence. The structural matrix does not close those gaps. M5 signing, Oracle deployment, rollback and public-domain checks have not run in this batch. No production data changed.
