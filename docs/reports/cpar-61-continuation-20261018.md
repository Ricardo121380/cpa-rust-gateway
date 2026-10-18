# #61 continuation: field decision and production Build cancellation

Date: 2026-10-18 (Asia/Shanghai). **PARTIAL / BLOCKED; keep #61 OPEN**.
This continuation preserves the previous [report](cpar-61-responses-20261018.md), field
qualification, rejection fixes, Full, dual-axis review and every historical failure.

## Baseline, authority and field decision

Actual initial branch `codex/prism-v4-delivery`, HEAD
`9f7c15f72615ba25ea7ba47f4e0e992780d4f4a8`, matching the user's reference without resetting.
Initial index was empty. The frozen dirty inventory has 49,041 entries: 48,981 regular files,
10 symlinks and 50 nested-repository directory entries. Regular-file bytes/modes and symlink
targets/modes are protected; directory metadata is not a recursive descendant-byte claim.
See [baseline summary](assets/cpar-61-continuation-20261018/baseline-summary.json) and its
compressed manifest. Only this run's increments may be committed.

Root/ancestor rules, CONTEXT, issue tracker, specified reports/review, ADR-0107/0109/0110/0111,
BC-PROVIDER-003/024 and BC-OBS-006 were read. Live #58/#61 bodies/comments and paginated native
dependencies were frozen; #59/#60 are CLOSED, #61 OPEN, #62/#63 OPEN and blocked by #61. No
other issue is advanced. No private capture or real account credential was read for research.

Read-only first-party research follows the existing actual cost candidate. Decisive source
hashes match the previous manifest; the release pipeline injects GROK_VERSION and SOURCE_REV
alone cannot bind that source to shipping 1.0.50. SSE cost semantics are evidenced, while shared
Responses JSON/SSE/profile applicability and server shape remain unproved. The concrete
[channel contract draft](cpar-61-cost-contract-draft-20261018.md) specifies exact paths, presence,
integer bounds, semantic independence, retention and conflicts, and labels its assumptions.
**The user chose to keep NOT_ADMITTED and wait for additional first-party authority.**
No test-only allowance, arbitrary-JSON success, transport/profile substitution or successful
metadata retention is implemented. Other extra counters, anonymous Usage and real aliases remain
independently blocked. Proposed missing/null/zero/i64 rules are not a granted server contract.

## Implemented correction and controlled seams

A public HTTP RED reproduced source/lease ownership persisting while a local Usage receipt was
held. The shared SSE pump now remembers validated buffered Usage and releases its source before
terminal/error confirmation; cancellation also releases the source immediately while allowing
the same already-started confirmation to finish. It neither drops/recreates that confirmation
nor submits a second companion. The existing writer retains its bounded queue, transaction,
2-second confirmation timeout, AEAD, schema 35, payload/reader, TTL and keys.

[ADR-0112](../adr/ADR-0112-cancelled-stream-source-release.md) and the two updated contracts record
resource lifetime versus local recording. Actual Request delivery stays cancelled/failed even
when the Attempt started successfully and validated Usage later commits. Native price stays
non-settlement; unread Usage remains unknown.

The new seam is public Responses HTTP -> production compiler/factory/auth and actual leased Build
adapter -> a test-only fixed-native-request socket handoff -> real UpstreamClientPool -> independent
owned TCP peer. The shim sends only synthetic native body to an explicitly admitted numeric
loopback address; it is not the fixed production TLS endpoint or real Provider. The public client
socket tests run an actual Actix HTTP server. Body-drop cancellation is separately exercised at
the public service's MessageBody boundary. Independent counts are recorded after the peer receives
the entire native request, not inferred from send calls or internal poll counters.

Production request URL/profile/mode and model validation remain unchanged. Tests retain three
allowed attempts and a spare candidate; no extra Attempt or native receive follows a semantic
failure/cancellation. A real serving lease is observed at 1 before cancellation and 0 afterward;
an explicit later public request succeeds on the same single-capacity native account.

## Matrix delivered independently of field admission

| Controlled scenario | Required observation |
| --- | --- |
| Public body cancellation and abortive real socket reset after text | Peer EOF/reset, lease release, cancelled Request, no unreceived Usage/bill; explicit later request succeeds |
| Cancel while standard Usage receipt is pending | Actual lease 1 -> 0 and peer close before releasing receipt; original validated Usage/one ledger retained, Request Cancelled |
| Cancel while rejected native Usage receipt is pending | Exact encrypted full Usage and owner readback survive; no extra upstream read or second submission; Request Cancelled |
| Physical HTTP body truncation | Headers already 200 and text delivered, exactly one legal failure, EgressUnavailable, no success/retry/unvalidated Usage |
| Well-framed body ending before SSE terminal | One StreamTruncated failure; unknown Usage stays unavailable; resources released |
| Actual idle deadline | Silent upstream fails through UpstreamClientPool, one failure and receive/Attempt, no fabricated Usage or ledger |
| Actual total deadline with continuing heartbeat comments | Total deadline ends a non-idle stream; one failure and receive/Attempt; peer closes and lease is released |
| Same-Attempt encrypted evidence conflict | Two controlled local companion submissions with identical counts and changed cost bytes use the real writer; conflict is rejected/quarantined, original full readback survives, public RecordingUnavailable once, one source/Usage/ledger |
| Missing key/writer, SQLite write lock and confirmation timeout | Existing #59 public JSON/SSE regressions remain required; they qualify rejected Usage only, not admitted success metadata |
| Failed frame plus tail in one transport chunk | Existing #61 public regression remains required; verified Usage is retained and separately delivered tail is not read |

The new four test groups cover nine controlled variants. No legal Build nonterminal Usage source
was invented: current Build validates Usage only at completed/incomplete/failed terminal snapshots.
Pending-receipt tests use the window after terminal decoding and before downstream terminal delivery.
Completeness `complete_usage_object` describes that received object while the Request stays incomplete.

## All 14 current acceptance criteria

| AC | Current outcome and remaining condition |
| --- | --- |
| 1 | PARTIAL: root/item/part/Usage/identity inventory retained; exact cost source and bounded shape draft supplied, JSON/profile applicability unproved |
| 2 | BLOCKED: no admitted metadata path; JSON success-before-bytes encrypted confirmation not implemented |
| 3 | PARTIAL: standard JSON/SSE two tool loops, intermediate/follow-up questions, native summary, Usage and model regression passed; admitted-metadata variants unavailable |
| 4 | Scoped PASS: existing public zero-receive pre-send rejection regressed unchanged |
| 5 | PARTIAL: strict unknown/shape/identity/order/terminal rejection, recording failure and new exact conflict are controlled; successful metadata-specific precision/write faults remain blocked |
| 6 | Scoped PASS: Responses summary preserved; no Messages omission or native-v1 |
| 7 | BLOCKED by evidence and explicit user decision: cost remains NOT_ADMITTED; no actual approved field positive/readback can be claimed |
| 8 | Scoped PASS: unexplained object/counters and real aliases remain blocked; no real/client claim |
| 9 | BLOCKED: no common admitted success policy or successful incremental metadata writer; shared strict JSON/SSE policy remains |
| 10 | PARTIAL: existing fragmented/interleaved text/summary/tool and JSON-parity regression passed; no admitted-metadata variant |
| 11 | BLOCKED for admitted success: no success-before-terminal proof; new rejection-conflict late failure and existing missing-key/write-timeout rejection are separate |
| 12 | Scoped PASS for the production-Build SSE cancellation/abort/EOF/timeout and pending standard/rejected Usage matrix above; successful admitted-metadata variant remains unavailable |
| 13 | PARTIAL: scoped cancellation/failure/explicit-recovery receive, Request/Attempt/Usage/ledger evidence added; admitted success metadata unavailable |
| 14 | PARTIAL: standard public SSE two tool loops/follow-up, strict unknown rejection and JSON regression retained; admitted-metadata variant unavailable |

Every required group must be satisfied to close #61. Scoped PASS, Full or Standards cannot
override blocked AC2/7/9/11 or make the partial groups complete.

## Validation and review status

Directed lifecycle matrix: **4 passed / 0 failed** after preserved feedback. The original public
RED and GREEN are retained separately. Initial compilation caught a test Result equality mistake.
Initial expanded socket test used FIN/write-half-close, which does not necessarily express an
aborted response client; the final test explicitly uses a real TCP reset, alongside separate
MessageBody cancellation. Another harness feedback expected zero quarantines despite injecting
one real conflicting companion, and observed only graceful EOF despite a legitimate peer reset.
Final oracles explicitly require one quarantine only for conflict, zero otherwise, and an actual
EOF/reset/broken pipe plus independent zero active leases. Assertions were refined to the intended
observable contract; no production deadline, terminal or accounting assertion was relaxed.

Initial directed Clippy rejected five expect calls in the new TCP peer; they were replaced with
explicit I/O result propagation. A second check found one redundant closure, then strict directed
Clippy passed. Public Grok factory regressions are **28 passed / 0 failed / 1 ignored**, including
#59 JSON/SSE key/write-lock/confirmation failures, #60 identity, the previous coalesced-failure
Usage repair, new lifecycle/conflict tests and the five-turn standard JSON/SSE tool regressions.
The private-capture test remains ignored. The 4 new groups are included in 28, not added again.

Final tests were also run on immutable source `2a95ba7f0427365615e5f518cd7d637bd9968548`, containing
the unchanged baseline production library and the exact final three test blobs. A real reset
reproduced the resource defect (**0 passed / 1 failed**); the fixed source passes. The first attempt
at that isolated reproduction stopped before testing because SPA dependencies were absent; an
offline lockfile install supplied them, without changing any source or production assertion.

Immutable Full **PASS: 45/45 steps**, Rust **1542 passed / 0 failed / 13 ignored**, 554.041 seconds.
Source `38443146c1528d081794156261f6859ad195e255`, tree `772742747fb119507815860bc86074f35782a564`.
The [source manifest](assets/cpar-61-continuation-20261018/full-source.json),
[Full steps](assets/cpar-61-continuation-20261018/full-steps.md) and
[validation summary](assets/cpar-61-continuation-20261018/validation-summary.json) bind all four
product/test blobs to the current candidate. Workspace Rust used loopback-only Seatbelt and four
threads; the independent non-loopback probe returned EPERM. Shared HTTP/JSON/SSE, native decoder,
storage/reader/CLI and same-chunk Usage regressions are included, as are strict Clippy, local
serve/agent, source/boundary/docs/contract/secret/whitespace checks, cargo deny and RustSec audit.
The Full validation worktree remained tracked/index clean after execution.

Full includes current user rule/document overlays for validation; those pre-existing changes are
excluded from this run's branch increment, including the pre-existing portions of two co-edited
indexes. Only this run's appended paragraphs are staged there. Four product/test hashes, rather
than an assertion that the final branch and overlaid Full tree are identical, establish parity.
This is local controlled validation, not remote CI, a release artifact or a deployment.

All 17 raw RED/GREEN/feedback/Full logs passed the repository's secret policy before compression;
the [archive](assets/cpar-61-continuation-20261018/validation-logs.tar.gz) was read back member by
member against [SHA256/length records](assets/cpar-61-continuation-20261018/validation-logs-manifest.json).
The raw originals also remain in the run's private temporary directory. Initial cross-device
rename preparation failed without moving a log, then verified copy/relocation succeeded.
[Preservation](assets/cpar-61-continuation-20261018/preservation-precommit.json) verified all 49,041
original entries; removing the two owned append suffixes restores their exact initial dirty hashes.

Final independent [Standards/Spec review](assets/cpar-61-continuation-20261018/review-final.md)
completed on baseline `9f7c15f72615ba25ea7ba47f4e0e992780d4f4a8` and committed candidate
`fc1c4755c05bdfa7d91812fd77b9494225cc161a`. Standards is **PASS_WITH_NIT: 0 hard violations,
1 P3 possible test duplication**; the optional test-only extraction is recorded without changing
the validated source. Spec is **PARTIAL/BLOCKED: 2 completion-blocking requirement groups,
0 new behavior bugs and 0 scope creep found by static review**. Actual field qualification and
successful pre-delivery persistence are still missing. Reviewers independently checked source/
archive/ownership correspondence but did not rerun tests. No passing old Full is relabelled as
validation of this continuation. Subsequent closeout changes contain documentation/receipts only;
all four product/test blobs remain identical to this candidate and its Full source.

## Completion boundary

One authorized [result comment](https://github.com/Ricardo121380/cpa-rust-gateway/issues/61#issuecomment-6251016058)
was posted to #61. Independent single-comment and paginated list/issue/dependency readback
confirmed the exact body, one new comment, OPEN state and unchanged other issues/edge identities
and states ([receipt](assets/cpar-61-continuation-20261018/remote-readback-final.json)). The initial
whole-object equality check failed solely because GitHub's two comment endpoints returned
different avatar URL query strings for the same author; all remaining fields and old comments
matched exactly. That failed receipt remains alongside the field-scoped verification; no second
write was performed. #62/#63 are not started. The next necessary
input remains first-party authority establishing the actual cost field's JSON/SSE/profile relation
and shape, or a later explicitly approved field-specific channel contract. The user currently
chose the first route. Once admitted, success projection/commit-before-bytes/terminal, full readback,
metadata tool-loop variants and its failure/cancellation qualification still need implementation.

Research, controlled protocol, actual client, real Provider, remote CI, release artifact, deployment
and production are separate evidence classes. Real Provider inference/account budget, actual
Codex/Pi/Claude Code, push, remote CI, deployment and publication are **NOT_RUN** in this continuation.
