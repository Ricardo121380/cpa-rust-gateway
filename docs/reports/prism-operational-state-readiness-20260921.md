# Prism operational state acceptance — 2026-09-21

Implementation under test: `0bceff6`. This is a bounded M4 acceptance batch with no production code changes, deployment, external-model use or real Provider inference.

## Real embedded gateway / EgoLite Chromium

Used the existing disposable gateway at loopback port 55204 and its synthetic accounts, requests and catalogs. No production records or credentials were read or modified.

| Flow | Action and observed result |
| --- | --- |
| API key permissions | Space toggled the first actual model checkbox, yielding `[true,false]`. Keyboard Enter on clear removed both selections. With a name but no allowed models, Create remained disabled. Cancel required explicit discard; no key was issued. |
| Failed request detail | Keyboard Enter opened an existing mock-stream failure. The drawer showed failed terminal state, one upstream attempt, observed timing, no billing record, and a diagnostic deep link. At 390×844 neither document nor panel overflowed. Shift+Tab from Close reached View diagnostics; Escape closed the drawer. |
| Invalid price input | Switching malformed JSON `[{` to table mode showed an alert and retained the exact input. Replacing it with valid entries recovered the editor. |
| Long price catalog | 512 synthetic entries with 100-character model suffixes produced 50 rows on page 1. Enter on Next moved to page index 1. Selecting page index 10 showed 12 rows, including exact final model 511. A native End-key attempt did not change the select value; it was not counted as a passing pagination action. |
| Mobile price editor | At 390×844 the 50-row editor did not overflow horizontally; both footer actions were 44px high. Cancel/discard removed the unsaved editor. No catalog was imported. |

These observations complement the earlier real mock request-to-billing chain and key reveal/clear receipts. They do not claim a new official OAuth authorization, new persisted price import, or exhaustive screen-reader certification.

## Fresh regression evidence

`npx playwright test e2e/billing-lifecycle.spec.ts e2e/billing-inline-ownership.spec.ts e2e/key-access-lifecycle.spec.ts --project=chromium --workers=2`

**32 passed, zero retries, 37.1 seconds.** Log: `/tmp/prism-state-readiness-20260921.log`.

Coverage includes 1/50/51/512 full catalog preservation, 513 rejection, 1024 differences, off-page invalid rates, global import vs draft policy distinction, capacity, held writes, lost-response non-replay, shared pending configuration, private key permissions, partial failures, lost key issuance and retired-session ownership. This run uses fixtures; real gateway observations are listed separately above.

No implementation changed, so the preceding type/build/embedded evidence remains applicable; those checks were not redundantly rerun for this report-only batch.

## Remaining readiness

The bounded API-key, request-detail and price-editor scenarios above now have current evidence. M4 still needs final consolidation of dashboard empty/error states, provider/model long/error variants, advanced-maintenance state navigation, and the historical real-channel/manual authorization evidence inventory. M5 signed release and Oracle verification remain pending. Do not interpret previous checklist wording such as “every variant” as an unbounded requirement; remaining cases must name a concrete behavior and expected outcome.
