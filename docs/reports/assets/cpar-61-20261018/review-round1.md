# Fixed-candidate review, round 1

Baseline: `f2453ff25d7b1c56d42dfb1d0cf26cba674d4db4`.
Candidate: `ad3019cb5e00e67b4d5e5263a3d413beff3c566f`.
Command: `git diff f2453ff25d7b1c56d42dfb1d0cf26cba674d4db4...ad3019cb5e00e67b4d5e5263a3d413beff3c566f`.
Parallel, independent read-only Standards and Spec agents; no tests run by the reviewers.
These original findings are retained even after a later correction.

## Standards

FAIL: one hard finding and one judgement call. No explicit product-code standard breach found.

- Hard: `full-source-final.json` copied the parent validation tree `76050848...` while naming
  source commit `6ff3fcc8...`. Its actual tree is `40963e4b...`. This contradicts the required
  current source identity. The original receipt is preserved as
  `full-source-final-reviewed-error.json`; corrected receipt resolves the tree from Git.
- Possible Duplicated Code (nonblocking judgement): repeated public fixture/App/request setup
  in `standard_responses.rs` and `tool_rounds.rs`. An extraction could reduce repetition.
  Left local to the tests: this is not a demonstrated behavior defect or a hard repo rule.

All 12 Fowler baseline smells were considered across the pinned diff; the repository rules
and tooling checks override those heuristics.

## Spec

FAIL: three finding groups, consisting of one behavior defect inferred statically and two
acknowledged delivery gaps.

- AC10/12/13: `push_bytes` atomically rolls back the whole chunk when a conflicting tail follows
  a valid failed terminal. The failed Usage parsed earlier in that chunk is lost. The reviewer
  explicitly marked this as an unexecuted control-flow inference. The primary subsequently
  reproduced it at the public production Build seam (`coalesced-usage-red.json`) and fixed it
  (`coalesced-usage-green.json`), retaining separate and coalesced cases without reading more.
- AC1 PARTIAL; AC2/7/9/11 BLOCKED: no actual common JSON/SSE/profile extra field has qualified,
  so metadata success retention/projection/readback is absent. Frozen first-party sources
  support SSE cost extraction but do not prove the JSON/profile contract. Source manifest
  hashes were independently verified. No synthetic allowlist can replace that evidence.
- AC3/5/10/12/13/14 PARTIAL: admitted metadata success write/ack-timeout/cancel matrix is absent.
  Standard five-turn tool success and failure Usage readback do not fill that gap.

AC4/6/8 are satisfied only within the declared controlled scope. No extra scope creep found.
Full was pending at review and was not counted as a code finding. Actual clients/real Provider
were intentionally not run and are not a suggestion to exceed authorization.

Findings per axis: Standards 2 (worst: incorrect receipt identity); Spec 3 groups (worst behavior:
coalesced failure loses verified Usage; completion blockers remain independently).
