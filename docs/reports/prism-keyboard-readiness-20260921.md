# Prism keyboard and state readiness — 2026-09-21

M4 remains in progress. This batch fixes a real shared keyboard defect; it is not a production deployment or complete accessibility certification. Base: `abbcd23` plus this batch. No Codex with ChatGPT or real Provider inference.

## Implemented and verified

The real embedded gateway reproduced loss of focus to BODY after Escape closed the provider inline editor. `InlineWorkspace` now retains its opener, waits for unmount, and restores it only when it remains connected and no replacement dialog or focused view owns focus. StrictMode cleanup does not restore focus while the editor is still connected. Route navigation keeps the destination link's focus.

- Chromium regression: the new Escape/cancel/route-focus case and two workspace visual cases passed (3/3, zero retries). A separate 12-case run covered inspector-to-editor transitions, mobile 126-character model identity wrapping, six price rates, read failure/retry with stale results retained, dirty form focus restoration, and pending-write close protection. These are separate runs.
- EgoLite on the rebuilt disposable embedded gateway confirmed the original Escape defect is fixed. At 1440×900, 1280×720 and 390×844 the opener regains focus and the document does not overflow.
- Actual account modal: Shift+Tab from close wraps to import; Tab wraps back. Dirty Kimi material → Escape → Keep editing restores the textarea; discard restores the account opener. Invalid synthetic Kimi OAuth material returned an explicit not-added result from the real gateway. No official authorization or real credential was used.
- All eight primary navigation entries were activated using keyboard Enter, reached their expected headings, and retained visible keyboard focus on subsequent Tab. This is navigation evidence, not certification of every control.
- TypeScript, deterministic four-file `check:full`, gateway compilation and three Rust embedded tests passed.

Logs: `/tmp/prism-keyboard-20260921.log`, `/tmp/prism-keyboard-focus.log`, `/tmp/prism-keyboard-type.log`, `/tmp/prism-keyboard-build.log`, `/tmp/prism-keyboard-gateway.log`, `/tmp/prism-keyboard-embedded.log`. Sanitized keyboard navigation observations: [JSON](prism-keyboard-workspaces-20260921.json).

## Workspace evidence and remaining scope

| Workspace | Evidence available | Still not certified by this batch |
| --- | --- | --- |
| Dashboard | Real keyboard navigation; preceding real mock-request metrics | Complete keyboard traversal of charts and empty/error variants |
| Accounts | Real modal trapping, dirty restore, invalid-import result | Official first/reauthorization requires its separately recorded channel evidence |
| Providers | Real inline close focus at three sizes; fixture stale-read retry | Every long resource name and empty/error layout |
| Models | Fixture long exact-ID mobile inspector and route/detail transition | Every source/candidate empty/error variant in EgoLite |
| API keys | Fixture metadata inspector and existing secret protections | Full keyboard traversal of permission selection and reveal receipt |
| Requests | Real keyboard navigation; preceding mock terminal chain | Every long retry/error detail variant in EgoLite |
| Usage/prices | Fixture six-rate inspector and preceding 512-entry timing | Full keyboard traversal of long price editor and all error states |
| Settings/advanced | Fixture egress pending-write and dirty close protections | All maintenance subpage keyboard/empty/error variants |

Next: finish the explicit gaps above with bounded scenarios, then reconcile M4 readiness and prepare the signed M5 release. No DNS, Caddy, Autoreg, production accounts or historical data changed. Local gateway restart briefly interrupted the browser; navigation was restored after readiness, not counted as an application failure.
