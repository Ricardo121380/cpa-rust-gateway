# #61 continuation: fixed-candidate dual-axis review

Date: 2026-10-18. Two independent read-only, one-shot reviewers completed.
Baseline: `9f7c15f72615ba25ea7ba47f4e0e992780d4f4a8`.
Candidate: `fc1c4755c05bdfa7d91812fd77b9494225cc161a`.
Recorded command: `git diff 9f7c15f72615ba25ea7ba47f4e0e992780d4f4a8...HEAD`.
Pinned equivalent: `git diff 9f7c15f72615ba25ea7ba47f4e0e992780d4f4a8...fc1c4755c05bdfa7d91812fd77b9494225cc161a`.
Commit list: `fc1c4755 #61 Release cancelled Build SSE sources before local recording completes`.
The fixed point resolved, the merge base matched it and the diff was nonempty before dispatch.
See [candidate receipt](review-candidate.json). These are lightly cleaned independent reports.

## Standards

**PASS_WITH_NIT; hard violations 0, heuristic findings 1, worst P3.** The reviewer examined
the four Rust files, this run's ADR/BC and reports, and checked the remaining indexes,
receipts and archive correspondence within the 70-file candidate.

- **P3 / possible Duplicated Code (judgment call):**
  `apps/gateway/src/runtime/batch_c/grok/standard_responses/lifecycle.rs:548,712` and
  `apps/gateway/src/runtime/batch_c/grok/standard_responses/lifecycle/recording_faults.rs:141`
  repeat `let worker = crate::billing_worker::BillingWorker::start(...)` through
  `worker.stop().await` and `SqliteBillingLedger::open_read_only(...).list_bounded(10)?`.
  A small helper could carry the common test preparation/ledger read while retaining each
  scenario's assertions. This is a Fowler baseline suggestion, not a documented hard breach.
- `crates/gateway-http-actix/src/lib.rs:3278,3321,3368` releases the source on terminal,
  cancellation and error; line 3326 awaits the original confirmation without a second
  submission. This follows ADR-0112 lines 24/29 and adds no Provider type dependency
  (crate-boundaries line 69). ADR-0109 line 24 explicitly establishes bounded confirmation;
  the older synchronous event-port wording is not an introduced violation.
- Independent candidate/Full/current SHA256 comparison matched all four code blobs;
  Full tree matched its manifest. All 17 archived members and the archive itself matched
  SHA256/length records. Original logs contain 45 steps, Rust 1542/0/13 and directed 28/0/1.
  The reviewer did not rerun tests.
- Both co-edited index commits equal baseline committed content plus the owned suffix;
  removing those suffixes from current files restores the initial dirty hashes. All 70
  committed paths match staging ownership. Full/Standards does not override NOT_ADMITTED
  or the report's PARTIAL/BLOCKED status.

Disposition: retain the validated code candidate. The P3 test-only extraction is not necessary
for this resource-lifetime correction; it remains recorded, without adding a speculative
refactor or treating it as a completion blocker.

## Spec

**PARTIAL/BLOCKED for the whole ticket; two completion-blocking requirement groups;
new behavior bugs 0 and scope creep 0 found by static review.**

- **Field qualification still blocks AC1/7.** The spec requires actual field source, shape,
  protocol applicability and public positive/negative/readback evidence, and says to remain
  BLOCKED without applicable evidence ([spec-61](spec-61.md) lines 21/27). Cost has narrower
  SSE authority; JSON/profile correspondence and server shape remain unproved. The
  [draft](../../cpar-61-cost-contract-draft-20261018.md) lines 19-51/98 stays unapproved;
  [field decision](field-decision.json) lines 2-7 records KEEP_NOT_ADMITTED. ADR-0107's
  direction was not treated as approval of the concrete field.
- **The success path still blocks AC2/9/11 and limits other successful variants.** The spec
  requires full persistence confirmation before JSON success bytes or SSE success terminal
  (spec-61 lines 22/32/34). That implementation and admitted-field variants are absent and
  acknowledged by the continuation report. Mapping remains: AC1/3/5/10/13/14 PARTIAL;
  AC2/7/9/11 BLOCKED; AC4/6/8 scoped PASS; AC12 production Build strict-path scoped PASS,
  with admitted-metadata variants unavailable. These are whole-ticket gaps, not new bugs.
- AC12/13's resource-lifetime repair matches cancellation release, no read-ahead and validated
  Usage retention (spec-61 lines 35-36). The implementation caches decoded Usage and drops
  terminal/error sources; cancellation during confirmation drops the source before awaiting
  the same future (`lib.rs:3278-3282,3317-3338,3368-3374`). Tests use actual RST and check lease
  zero, one receive, Cancelled, full original readback and one ledger (`lifecycle.rs:438-556`).
  Conflict preserves original evidence and emits one failure (`recording_faults.rs:68-149`).
  No invented nonterminal Usage sample supplies missing field authority.

The reviewer read the four Rust differences, full new tests and specified spec/report/ADR/BC
sources; no test was run by the reviewer. Full cannot cover these blockers. Actual clients,
real Provider and publication remain NOT_RUN.

Standards: 1 heuristic finding, worst P3; Spec: 2 completion-blocking groups, worst missing field qualification and success persistence, with 0 new bugs and 0 scope creep found.
