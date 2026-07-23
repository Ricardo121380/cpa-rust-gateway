# G9 Grok Web local phase-gate report

| Field | Value |
|---|---|
| Plan | `v1.44` |
| Gate | `G9` local closeout |
| Date | `2026-07-23` |
| Verification branch | `codex/p8-official` |
| Local result | `PASS` — P9-01 through P9-09 met local acceptance and the authorized Canary supplied the required live Web evidence. |
| Remote Delivery Gate | `PENDING` — the next and only P9 delivery event is the annotated `phase-p9-complete` tag. |

## Conclusion

P9's local phase gate passes. The separate Web credential/session boundary, exact egress binding,
fixture-only parser, Conversation state, signer cache, quota projection, 403 ownership, and
default-off Tool emulation retain their local evidence. The bounded P9-09 Canary then supplied
three controlled observations: a direct local 4xx classified only as egress/WAF-shaped, an
accepted server-egress Conversation response, and an accepted server-egress full Canonical text
lifecycle.

This is a local conclusion, not yet P9 `DONE`. The phase branch must receive one code closeout
commit and only the annotated `phase-p9-complete` tag may trigger GitHub Fast, Full supply-chain,
and Required. P10 must not start, merge, deploy, or release before that run passes.

## G9 conditions and evidence

| G9 condition | Evidence | Local result |
|---|---|---|
| Web credentials, egress sessions, Conversations, and Cookies remain separate from Build/Official | P9-01/P9-02/P9-04 type and state-isolation tests; the temporary Canary import used only the Web SSO lifecycle. | PASS |
| A generic 403 does not ban an account | P9-07 exact ownership matrix plus probe 1's value-free egress classification. | PASS |
| Current Web request/response contract reaches an admitted Canonical lifecycle | P9-09 probe 3 through repository transport, live JSON-object decoder, and one text lifecycle. | PASS |
| Protocol drift does not silently succeed | P9-09 decoder requires identity, final envelope, monotonic text, valid EOF, and no post-final data; rollback/identity/final regression tests pass. | PASS |
| Tool emulation stays explicit and disabled by default | P9-08 and P9-09 fixed Tool-free request body; no production Feature Flag was changed. | PASS |
| No secret or reference-runtime leakage occurs | Secret scan, redacted diagnostics tests, focused source review, and no copied grok2api implementation. | PASS |

## Local verification and review

The P9 local Full gate passed with shell/CI/plan guards, cached-tool verification, formatting,
workspace Clippy/tests, source and crate-boundary policy, document links, tracked Secret scan,
dependency policy, and RustSec audit. The ignored P3/P4/P5/P6/P8/P9 live harnesses remained
ignored during that local gate.

Focused phase review found and fixed one release-blocking behavior before the gate: the live
decoder used to accept a final message that was a prefix of text already emitted. That contradicted
the P9 fail-closed text-rewind contract. It now accepts only equal final text or strict suffix
extension; the focused suite covers the shortened-final regression, Conversation identity change,
and duplicate final envelope. No other release-blocking finding remains.

## Delivery boundary

The phase closeout must not issue a fourth Conversation request, re-run a live Canary, import a
credential into storage, change a server, change proxy/TUN routing, enable a production flag, or
begin P10. It may commit the reviewed artifacts, push the existing Phase branch, and create the
single annotated P9 tag. On remote success, reconcile P9-01 through P9-09 and G9 to `DONE`; on a
remote failure, stop P10 and repair only the failed closeout target.
