# G6 Grok Build phase gate report

| Field | Value |
|---|---|
| Plan | `v1.37` |
| Gate | `G6` reopened closeout |
| Date | `2026-07-23` |
| Verification branch | `codex/p6-grok-build` |
| Local closeout target | Pending the reviewed closeout commit and annotated `phase-p6-remediated-complete` tag |
| Local result | `PASS` — all P6 Tasks are `LOCAL_PASS_PENDING_PHASE_GATE` |
| Remote Delivery Gate | `PENDING` — exactly one new tag-triggered Fast + Full supply-chain + Required run remains |

## Reopened acceptance conclusion

The old `phase-p6-complete` run remains evidence only for its historical closeout; it cannot prove
the original missing production `InferenceAdapter` or C28's true direct dual-mode lifecycle.
The repaired P6-03 boundary now uses explicit injected credential/transport state and a
Router-facing mode bridge. Its real direct validation has one non-streaming pass (T26) and one
SSE pass (T33); both produced Canonical `ResponseStart`, text, and a clean `ResponseEnd` without a
`StreamError`. The intervening diagnostics and compatibility work remain structurally redacted.

P6-04 through P6-08 were then revalidated in dependency order. They are local passes pending the
same single P6 Delivery Gate. No P7 work, server/account/route/proxy/TUN mutation, or extra live
Provider tuple is part of this closeout.

## G6 conditions and evidence

| G6 condition | Evidence | Local result |
|---|---|---|
| Direct Build supports both required response modes through the executable Provider chain | P6-03 Adapter and Router fixture E2E cover both modes and mode selection; T26 non-streaming and T33 SSE are independent real Canonical lifecycle passes. | PASS |
| Two Credentials stay isolated through refresh/runtime state and quota observations | P6-02 CAS/singleflight evidence plus reopened P6-04 exact-Credential catalog/quota tests and version-7 migration up/down checks. | PASS |
| Cache Identity/Affinity is stable and never crosses Client Key scope | Reopened P6-05 tenant-isolation/rebind test requires durable break evidence and preserves exact HMAC-derived scope. | PASS |
| Response continuation never silently changes account | Reopened P6-06 test proves exact owner binding, cross-tenant absence, AEAD replay isolation, and controlled clearing. | PASS |
| Old request state cannot overwrite a newer credential or permanently poison it after transient/egress failure | P6-02 CAS evidence plus reopened P6-07 401/403/429/quota/408/5xx fail-safe action matrix. | PASS |
| Clean-room comparison stays structural and source-independent | Reopened P6-08 report review and boundary check found no source import, `include_str!`, or path/git dependency on reference projects. | PASS |

## Verification and independent review

`./scripts/check.sh full` passed after the complete reopened closeout: shell/workflow and plan
guards, format, workspace Clippy/tests, source policy, Secret scanner, crate boundaries, document
links, whitespace, fixed-tool version policy, `cargo deny`, and RustSec audit. The full test set
includes P6 OAuth, refresh, strict decoder, probe safety, runtime continuity, and new
Provider-to-Router fixture suites.

Independent diff review found no release-blocking issue. It specifically checked that the
transport is egress-admitted before send, response headers cross the adapter only as safe
classifications, bounded body reads do not retain values, known SSE extensions validate identity
and final-text consistency, and unknown/refusal/contradictory event shapes remain fail-closed. The
new `gateway-router` relation is a documented `provider-grok` dev-dependency used only by the
fixture vertical test, not a runtime Provider-to-Router edge.

## Delivery boundary

After the reviewed closeout commit is pushed, the annotated `phase-p6-remediated-complete` tag will trigger
the only new formal P6 delivery run. Fast, Full supply-chain, and Required must all pass before G6
or any P6 Task becomes `DONE`. No pull request or docs-only closeout run will be created. P7
remains out of scope until the user explicitly starts it.
