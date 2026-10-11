# CPAR-38 / #47: Chat JSON 502 diagnosis and evidence-preserving repair

The actual review base is `5a7826e806a04b3d403b1b1c0eef00a2b686ff32` on
`codex/prism-v4-delivery`. The user's 2026-10-11 request authorizes this bounded
slice and updates to the existing progress records of #47, #57 and #9. All three
issues remain OPEN. Prior checkpoints are evidence, not rollback instructions.

## Product and local evidence

Trace the actual public path: Build decode, canonical/native events, response
projection, HTTP collection/encoding, Request/Attempt/Usage and billing. Preserve
safe stage and stable rejection reason without exposing client content, native
values, credentials or headers in public errors or ordinary logs. Attach an
existing Request ID on applicable failures; never invent an identity before its
admission boundary. Preserve only validated source Usage, exact Attempt ownership,
non-additive merging and idempotent ledger semantics. Missing facts stay unknown.
Attempt success and final Request failure remain separate outcomes.

Diagnostic capture defaults off and is restricted to this candidate and one
explicitly selected synthetic request. Prefer enums, structural field summaries
and necessary digests. Any native content needed for diagnosis stays in an owned
0700/0600 private directory, excludes credentials/authentication headers, and is
never committed, posted to GitHub, logged normally or sent to an external model.

Reproduce supported defects locally before the smallest repair. An offline fixture
from a new real sample proves only that new epoch, not the uncaptured old response.
Keep native items, reasoning, citation, signature, tool result, call_id, order,
history, declared capabilities and the 4096 limit. Equivalent representations may
be repaired. Unrepresentable semantics retain an explicit specific rejection;
changing an accepted rule needs a focused user decision, never an automatic
exception. No `web/prism/**` edits are authorized in this slice.

Validate success/failure through public HTTP, including safe reasons, correlation,
known/unknown Usage, duplicates and ledger behavior, lossless/strict projection,
and affected Chat/Messages/Responses JSON/SSE regressions. Run applicable Task and
Phase gates at immutable new source, plus independent complete Standards and Spec
reviews. Reuse old evidence only when its source and scope remain identical.

## Conditional new real diagnostic epoch

Real execution is permitted only if existing evidence cannot establish the exact
failure and the diagnostic candidate has passed local gates and both complete
reviews. Reuse the approved isolated Linux build recipe and existing dependencies;
a changed product requires a newly built source-bound candidate. Use existing
new-vps, the same approved Build account/source, grok-4.5, an owned new directory
under the existing protected acceptance root, a new epoch and temporary instance.
Production service/current/config/Caddy/ports are outside authorization.

New limits: one new Pin, then only on PASS one original Chat JSON request, total
at most two inference intents. Historical intents35 and gateway/Pin-confirmed
sends34 are separate. The previous unused58 are terminated. Monetary policy does
not relax the request or stop limits. Reserve intents durably before transport;
reject replay. Pin failure, uncertain identity/evidence or qualification failure
immediately stops the epoch. After Chat, stop regardless of result. No retry,
fallback, proactive refresh, reauthorization, account replacement, supplemental
sampling, other protocol/extension, or post-repair real retest. No reproduction
means NOT_REPRODUCED, with no additional sample.

Before send, re-freeze actual source/binary/manifest, process/start identity,
configuration, route/candidate/model, Client Key, account material, independently
observed lease, egress, serving declarations and expiry. Require one account,
one candidate, max_attempts=1, background egress disabled, guard/audit and exact
remote bytes matching the reviewed freeze. Old lease/Pin/session qualification
never applies. Normal refresh continuity uses only BC-RESP-003's existing proven
same-authority rule. Independent Provider reception and actual cost remain UNKNOWN
without independent evidence.

## Exit and delivery

If used, stop only the owned temporary process, verify both ports released and
post-stop audit unchanged; verify production baseline unchanged, Colima restored,
history preserved and private modes intact. Preserve user AGENTS.md and all
unrelated paths. Stage/commit owned paths only, with no reset/cleanup/broad add,
push, formal release/deployment or production acceptance.

Append sanitized results to each existing GitHub progress comment while preserving
its full historical prefix, and independently read back comments, issue body/AC,
state, metadata and paginated dependencies. Separate root cause/hypotheses/unknown,
product/local/review/candidate, real diagnostic/full matrix/historical acceptance,
intent/send/unknown reception-cost/exit, remote CI/release/deployment/production.
Finish with conditions and recommended budget for a future basic matrix; do not
start that future epoch automatically.
