# Read-only exploration receipt

Two generic scouts were spawned concurrently with explicit `fork_turns=none`, no
role/model override, and one self-contained read-only task each. Main thread waited
while they ran; no delegated-scope files were edited. Main thread owns all changes,
decisions and executed validation.

- `/root/b03_public_test_map`: terminal result `blocked/PARTIAL`. Located B-02
  fixture, public HTTP/storage/WS oracles, actual network/handoff counts and B-01/
  done/tool regression names. A final `rg` included a nonexistent report directory
  and returned exit 2; scout ended without further verifying the Grok tool helper.
  Its file:line references were used for focused reads by the main thread. This is
  exploration, not a test or independent review PASS.
- `/root/b03_order_boundary_map`: no terminal review result; after bounded waits,
  one request to stop expanding and return partial results, it was interrupted.
  No conclusion was used. Main thread then read the relevant native encoder and
  decoder code directly. This interruption is not counted as independent review.

Both scouts made no source edits or runtime test claims. Later two-axis reviewers
are separate new one-shot agents with fixed commit and complete diff coverage.

A later new one-shot generic scout `/root/b03_fallback_fixture_credentials`
returned terminal `complete`, static-only. It located the HTTP 503 / zero-attempt
fixture rejection at `attempt_orchestrator.rs:1703-1719`: the mapped public-model
entry requires a unique Provider, and the two synthetic upstream IDs made it
ambiguous before driver/lease selection. Main spot-read `runtime.rs:2045-2083`
and `gateway-http-actix/src/lib.rs:2779-2788`; the existing public exact-upstream-model
entry permits the intended two-candidate retry control. Main chose that existing
entry (`model: upstream-c`) instead of the scout's same-upstream suggestion,
which would conflict with the compiler's one-endpoint-per-API-format rule.
Main then executed actual primary/fallback network and handoff count checks.
The scout did not modify files or run tests, and its result is exploration only.
