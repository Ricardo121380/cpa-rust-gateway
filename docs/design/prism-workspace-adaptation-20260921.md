# Prism shared workspace adaptation — 2026-09-21

Source: the user-approved OpenDesign K3 **high** exploration in `prism-workspace-20260920/`; provenance and rejected prototype behaviors remain authoritative. No new external model or C2C request was made.

## Production choices

- Palette: existing silver canvas `#e8ebf0`, white surface `#ffffff`, graphite canvas `#111419`, dark foreground `#f2f4f8`, light foreground `#1b1f26`, Apple blue `#0066cc`. Editor surfaces derive from current theme tokens rather than introducing a second palette.
- System typography: page titles 24px, editor titles 18px, form labels 13px/500, input values 14px/400 and 16px on phones. No CJK negative tracking.
- Layout: title and close action → labelled content → fixed action hierarchy. Desktop inline footer clears the existing draft dock; mobile footer stays in document flow. Actual labelled fieldsets keep their grouping; unlabelled fieldsets used only to disable controls lose decorative nested borders.
- Inline workspaces use a tinted header/footer and readable content plane, not another backdrop blur. Preserve the existing three glass chrome surfaces, shared workspace material, drawer geometry, focus ownership and session cleanup.
- Move inline styles out of billing into `src/design/workspace.css`, loaded once after established design layers. Explicit shell-scoped rules prevent historical underlap selectors from removing left padding.

## Verified this batch

- 14 existing contrast/inline/modal/egress browser cases and 2 new visual-geometry cases passed.
- Real embedded gateway, EgoLite: inspected dark desktop provider form, light 390×844 provider form, and inline policy editor at 1440×900, 1280×720, 390×844. Controls in the phone modal are 44px tall; phone input text is 16px; inline padding is balanced and no document horizontal overflow was observed.
- Reduced-transparency emulation produces an opaque editor without shadow. Motion/focus regression coverage remains in the existing modal tests; this is not whole-app accessibility certification.
- Local screenshots: `/tmp/prism-form-before-20260921.png`, `/tmp/prism-form-after-dark-20260921.png`, `/tmp/prism-form-after-mobile-20260921.png`, `/tmp/prism-workspace-mobile-final-20260921.png`. These are synthetic local data, not production screenshots.

## Remaining gaps

- `ProviderDialog.tsx` still uses automatic `beginConfigurationTask` completion and says “保存并应用”; creation needs the confirmed pending-batch lifecycle and explicit outcome.
- `CandidateDialog.tsx` still places complex source/capability editing in a Sheet. Route deletion/short route settings should retain the appropriate simple confirmation/form; do not indiscriminately turn every Sheet into an inline editor.
- Final all-workspace dark/light, keyboard, long-content, empty/error states and performance acceptance remain open.
- Actual draft navigation displayed a generic conflict banner during some reads. Cause is not established in this visual batch; investigate before final acceptance rather than hiding the banner cosmetically.
