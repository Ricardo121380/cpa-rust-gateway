# Local draft for #57 — B-03 / P2 evidence only

This is a local draft, not submitted to GitHub. It does not close #57 or release.

B-03 release-readiness finding is **RESOLVED_LOCAL** on source
`5e51f5f170a01b813631824b004eb2fcbf190201`, fixed start
`9265e6cea83b6172742dc2e4e60b5e5d94c1a1c2`.
Local implementation commits are `edb4a93f8689ea3986917ec8befbeec297260889`
and `5e51f5f170a01b813631824b004eb2fcbf190201`, both titled `B-03: ...`.

- Final native output array positions must agree with established item IDs/indices
  and exact completed metadata. Conflicting terminal order fails before ResponseEnd;
  no sort/rebuild/state overwrite hides it. Legal delta and item-done interleaving
  and permitted omissions remain supported.
- Original public RED had valid B-02 confirmations and only reversed final output.
  The gateway incorrectly completed and reordered it; actual cargo exit 101 is saved.
- Additional two-candidate RED proved bootstrap replay to a usable fallback:
  primary/fallback received 1/1, Attempt 2, incorrect HTTP200 completed. The minimal
  Responses bootstrap correction keeps UPE nonretryable. GREEN is HTTP502 UPE,
  received/handoff 1/0, Attempt 1. Transport EOF positive control remains 1/1,
  Attempt 2 and successful completed.
- Public HTTP/SSE, storage GET/continuation and real loopback WebSocket verify
  permanent failure, RequestFailed, no successful stored result or usable WS root,
  and exact positive output/content/metadata/history retention. AttemptSucceeded
  still means a source was handed off, independently of final request outcome.
- Directed batch_c including B-01/B-02/tool regressions: exit 0, 26/0/0.
  Protocol/router: exit 0, 224/0/1 preexisting ignored. Strict Clippy: exit 0.
- Exact-source clean detached checkout Full: exit 0, 45/45 steps,
  1477 passed / 0 failed / 12 ignored; before/after clean, source hashes unchanged.
- Independent Standards PASS and Spec PASS, both composed COMPLETE 15/15.
  Standards title violation was fixed and independently closed; one nonblocking
  testing-helper duplication advisory is explicitly retained. Interrupted/PARTIAL
  probes and prior FAIL remain distinct evidence, not PASS.

Delivery report: [B-03 validation](../../cpar-b03-responses-output-order-20261008.md).
Structured source/test/review receipts: [verification.json](verification.json).
Raw reviews: [review-final.md](review-final.md).
Full receipt: [full-receipt.json](full-receipt.json).

Only B-03 is newly resolved. Original P2 remaining: 9
(A-02–A-08, B-04, B-05). S/E/O findings are tracked separately and untouched.
Suggested next local slice: B-04 bootstrap validated Usage ownership; this source
does not repair its Usage carrier. Release remains BLOCKED.

No real Provider/account inference, remote CI, new-source artifact/signature,
deployment, production acceptance, push, publication or GitHub write was performed.
The old source80 artifact/signature/remote validation cannot be inherited.
