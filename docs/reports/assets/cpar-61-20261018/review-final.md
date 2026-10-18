# Fixed-candidate review, final

Baseline: `f2453ff25d7b1c56d42dfb1d0cf26cba674d4db4`.
Candidate: `d4ac176b149bf427820235b4a69902cd9fa4d340`.
Diff: `git diff f2453ff25d7b1c56d42dfb1d0cf26cba674d4db4...d4ac176b149bf427820235b4a69902cd9fa4d340`.
Commit: `d4ac176b #61 pinned review-final-candidate: strict Responses failure integrity; metadata admission BLOCKED`.
Two independent read-only agents reviewed in parallel. No tests or real Provider calls were run
by the reviewers. All six product blobs are frozen; subsequent changes only record these review
results and final validation/preservation receipts.

## Standards

**Standards verdict: PASS; current hard violations 0, judgement 1 (nonblocking).**
Scope: six Rust candidate Git blobs, related ADR/contracts/reports and key receipts. Existing
working-tree rules were used as standards sources, not counted as this run's increment.

The first-round hard finding is resolved. `full-source-final.json:2` names source
`6ff3fcc8b6ee3d31f4406b91e66703601bf378ae`; line 4 now has its independently resolved tree
`40963e4b2ec276a1e606636f50e9e31ac5ddc54e`. The prior error is explicitly preserved in
`full-source-final-reviewed-error.json:4` as historical evidence. The new source
`8ea7d88086d219dbdb91348335bc4fd31fb56411` has tree
`d14846c1b7228aaca8c2dd621eec48b32cd55291`, matching `full-reviewed-code-source.json:2–3`.
All six product SHA256 values were independently recomputed against the candidate and validation
source. The archived `logs/full-reviewed-code.log` hash matches `full-reviewed-code.json:17`.

**Possible Duplicated Code**, low severity and nonblocking: `standard_responses.rs:95`, `:167`,
`:367` and `standard_responses/tool_rounds.rs:160` repeat
`P12TransportProfiles::try_new()?` → `profiles.build = Some(...)` →
`endpoint.transports = Arc::new(profiles)`, followed by `test::init_service(App::new()...)`.
This is a test-setup maintenance heuristic, with no matching hard repository prohibition or
observed behavioral defect. Keep the first-round nonblocking assessment; unrelated extraction
is not required in this run. All 12 Fowler baseline smells were considered, with repo rules and
tooling taking precedence.

Limit: no tests or real Provider access by this reviewer. The report retains PARTIAL/BLOCKED/OPEN;
field admission, success retention and complete Build cancellation/timeout qualification remain
undelivered. Standards PASS and Full do not remove those boundaries.

Standards findings: **1 (hard 0, judgement 1)**. Worst current item: nonblocking possible
Duplicated Code.

## Spec

**Whole-issue verdict: PARTIAL / BLOCKED; cannot mark complete.** The fixed diff, all 14 AC and
parent specification were reviewed without running tests.

1. **Actual-field qualification and successful persistence are missing.** Spec `spec-61.md:27`
   requires at least one actual qualified field plus complete readback; line 22 requires
   persistence before successful bytes. All five public source files independently match the
   manifest. Upstream `2bdd1d6a` sampler `client.rs:137` extracts positive i64 cost only for SSE
   completed/incomplete; `:1374` JSON merely deserializes typed Response; `:560` also gates
   versions. They do not prove common qualification for actual JSON/profile 1.0.50. Candidate
   field report line 71 explicitly states no metadata success store/reader. AC1 PARTIAL;
   AC2/7/9/11 BLOCKED; AC3/5/10/13/14 lack the qualified-metadata variant. #59 failure Usage
   retention does not fill this gap.
2. **Complete Build cancel/timeout and metadata success fault matrix remain unproved.** Spec
   `spec-61.md:35` requires cancel/disconnect/timeout stop work and release resources; line 34
   requires one legal failure for write failure/confirmation timeout. Main report lines 67 and
   115–119 retain these gaps. Generic socket/drop and storage-timeout regressions cannot be
   promoted to PASS for this matrix.

The first-round behavior finding is **resolved**. `build_responses.rs:1045` stops and commits
Usage after a fully validated record emits StreamError; `:2136–2155` still validates identity,
root, status and Usage first. `inference.rs:552` finishes the source, preventing later reads at
`:590`. Success still uses whole-chunk transaction validation. `standard_responses.rs:211`
covers four variants; lines 288–345 check one failure, independent receive count, Usage, readback
and one ledger row. Already coalesced tail bytes and additional reads are correctly distinguished.

No scope creep, new false success, missed validation or extra reads were found in the six Rust
increments and reviewed failure paths. AC4/6/8 and standard five-turn regressions have only their
declared scoped conclusions.

Spec findings: **2 completion-blocking groups; new behavioral bugs 0**. Worst item within this
axis: actual-field qualification and successful metadata persistence are still undelivered.
