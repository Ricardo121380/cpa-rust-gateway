# Spec delta review: af81d911 to 66330dc6

Reviewer: /root/historical_send_count_spec; independent read-only FINAL_ANSWER, complete.

Spec delta: PASS; hard findings 0; original P2 independently confirmed resolved. Scope only the two-file af81d911...66330dc6 delta.

Facts:
- scripts/acceptance/cpar_new_build_acceptance.py:858 moves existing sample counting before preflight; :859-863 still preserve failure status; empty scenarios count 0. :867-870 preserve historical default version and scenario scope; sending/sample append paths unchanged.
- scripts/test-cpar-new-build-acceptance.py:276-283 covers v2 seven and v3 nine sends with observation 11 minutes stale: BLOCKED/runtime_observation_stale and count preserved; :268-274 retains historical scope assertions.
- output/cpar-preacceptance-20261010/stale-count-red.log:9 shows 0 != 7; final-clean-oracle.log:36-38 shows 33 tests / OK. No tests run by this reviewer.

Inference: the original P2 early return resetting observed sends to zero is removed; no sending or historical count scope expansion, matching request.txt:130 and :180-181. Original failed review remains in review-spec-af81-delta-failed.md:5-7.
