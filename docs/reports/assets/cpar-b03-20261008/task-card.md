# Task card: release-readiness B-03 / P2

Status: RESOLVED_LOCAL. Fixed base: `9265e6cea83b6172742dc2e4e60b5e5d94c1a1c2`.
Final accepted source: `5e51f5f170a01b813631824b004eb2fcbf190201`.
Exact-source clean Full and separate composed COMPLETE 15/15 Standards/Spec passed.
One nonblocking testing-helper duplication advisory is retained explicitly.
This finding is not CPAR-03 or the whole Batch B. B-02 accepted source remains
`15babae64154f8ff5fa1055f856d11640385633a`.

Scope M: one protocol crate, the existing Responses bootstrap boundary, gateway
test composition and its existing contract. Expected source files:
`upstream_response.rs`, `runtime.rs`, `batch_c.rs`, `batch_c/http/responses_done.rs`,
new child `responses_done/responses_order.rs`, and BC-PROTOCOL-001 supplement.
Evidence and local #57 draft remain under this asset directory and the B-03 report.
No schema, Canonical vocabulary, general retry policy, dependency, frontend or
production change. Responses bootstrap retains explicit protocol errors as
nonretryable failures; transport-only bootstrap EOF retains its existing behavior.

Dependencies: B-02 valid five-class confirmations/permanent rejection; B-01 multipart
history; #9 parent, #27 ordered public semantics, #31 exact native metadata.

Invariants: item identity and established output position agree with each supplied
terminal array position; no sorting or reconstruction hides contradictions;
legal cross-item delta/completion interleaving remains valid; legacy omissions
remain supported; failed flow cannot persist or register a reusable WS root;
actual upstream/handoff counts prove no replay; RequestFailed and AttemptSucceeded
retain their distinct existing meanings for a handed-off stream.

Test seams are explicitly authorized by the user and #9: public HTTP/SSE with a
real loopback upstream, owner GET/continuation and actual loopback WebSocket.
Public decoder byte-chunk checks supplement these paths.

Execution: unchanged-product public RED with actual cargo exit -> minimal correction
-> focused positive/negative and batch_c B-01/B-02/tool regressions -> protocol/router
tests and strict Clippy -> fixed source commit -> clean detached checkout Full ->
parallel independent Standards/Spec with complete fixed-base diff -> any correction
and required recheck -> local report/#57 evidence draft and documentation gate.

Budget target M <=25 minutes is a process target, not an acceptance deadline.
This slice exceeds it: required source/spec inspection, RED evidence, expanded
public storage/WS validation, and bounded read-only exploration waits are retained;
no test or review gate is reduced. The Task card was persisted after initial reads.

Forbidden: real Provider/account inference, credentials/production/service changes,
deployment, publication, push, GitHub writes/closure; A-02–A-08, B-04/B-05, S/E/O fixes.
Original AGENTS.md and unrelated files stay outside commits. Overall release BLOCKED.
