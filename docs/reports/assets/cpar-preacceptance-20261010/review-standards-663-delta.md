# Standards delta review: af81d911 to 66330dc6

Reviewer: /root/historical_send_count_standards; independent read-only FINAL_ANSWER, complete.

Standards: PASS; hard 0, smell 0. Scope only the two-file af81d911...66330dc6 delta, checked against all 12 supplied smell categories.

Facts:
- AGENTS.md:7: Codex owns everything except web/prism/**; both files are backend-owned, no cross-boundary changes.
- scripts/acceptance/cpar_new_build_acceptance.py:857 explains moving counting before preflight; :858 preserves the existing count expression and removes the later duplicate assignment.
- scripts/test-cpar-new-build-acceptance.py:276 names the regression directly; :277 reuses fixture in a loop for (2, 7), (3, 9); :281-283 check BLOCKED, reason and count.

Review judgment: no new naming, duplication, responsibility or abstraction issue in this delta. git diff --check PASS. No tests run by reviewer; no other historical/full-project review claimed.
