# Independent Standards and Spec review

Fixed diff: `git diff 189735ea3c281d770be6ad7ec159a5bfbdf7e2f8...67db3304b901b5c8308c7b4fed99dd4e926e66ed`.

Commits: `3722c2c3` (transport and regressions), `67db3304` (C4 fixture uniqueness). Scope: all three changed Rust files. The 18 inherited dirty/untracked documents were excluded from owned changes. Both independent agents were read-only and ran no tests. Review occurred while the final Full was running; static PASS does not pre-approve its result or replace the final source/preservation checks.

## Standards

**PASS (static), 0 actionable findings: 0 documented-standard violations, 0 heuristic smells.**

- AGENTS ownership/boundary rules: all three files are backend-owned; no listed shared surface was modified, so no cross-boundary log requirement applies.
- `crates/gateway-upstream/src/upstream_client.rs:570-577`: native roots are conditional on the admitted scheme. Cache identity includes scheme (`:540-556`); redirect is disabled (`:576`); supported proxies are Direct/SOCKS5, with HTTPS proxy rejected (`:189-212`). No supported HTTP path was found that would require TLS roots after this condition. The cold regression reuses the existing slow-body peer and verifies cold pool, partial content and terminal failure (`:1117-1154`).
- `apps/gateway/src/runtime/batch_c/grok/standard_responses/lifecycle/messages/cancellation.rs:247-303`: explicit status, delivered content, connection close, event uniqueness and empty-ledger assertions preserve deadlines and strengthen the existing oracle. No actionable duplication needs extraction.
- `apps/gateway/tests/cpar62_request_metadata.rs:24-55`: PID/time/atomic uniqueness remains a fixture helper. The fixed-time regression checks distinct paths, 0700 and an initially absent database; no production API was added.
- The changed hunks show no forbidden unsafe/unwrap/expect/panic/TODO/FIXME under Cargo lints and source-policy rules. Tool-enforced checks remain separate evidence.

The Standards agent did not claim Full, Spec, actual-client or Provider qualification.

## Spec

**PASS (static code and evidence statements), 0 actionable findings. Overall completion was PENDING at review time.**

- Task scope requires preserving deadlines and forbids raising them, unproved warmup or moving total. `upstream_client.rs:459-467` still starts total before cold construction; `:570-582` changes only native-root admission by scheme, with redirect/retry/timeouts intact. Cache scheme and allowed proxy boundaries support the condition. The new cold test keeps idle200/total500, status200, partial output and continuing failure (`:1117-1154`).
- Responses original assertions remain at `lifecycle.rs:865-925`. Messages assertions (`messages/cancellation.rs:247-303`) plus `messages/failures.rs:82-94` cover unique error and no success terminal. The report accurately identifies Actix test service with real upstream TCP, without public-listener qualification.
- `diagnostic-stress-1-stages.json:632-677` records cold765.986ms, pre-execute total exhaustion and the independent receiver0 snapshot. `comparison.json:424-442` retains historical Full attribution as inference and historical Messages cause as UNKNOWN. Receiver0 is not promoted to zero bytes/effects/cost.
- The C4 fixture addition is within the regression closeout: fixture-only atomic suffix and same-timestamp private-empty-directory check; no production storage, crypto or confirmation deadline changes. Deterministic RED is in `c4-directory-collision-red.log:295-303`; GREEN is in `c4-directory-collision-green.log:7-16`.
- Full requires all steps to pass. At review time `final-full-v2-check.md` had reached Clippy and the report draft still explicitly marked Full/source/preservation as pending. Those were remaining process gates, not static implementation findings. The first new Full FAIL was still retained in `final-full-check.md:41`. #62 and actual-client/Provider completion were not claimed.

## Separate completion gate

The primary agent must independently read the completed final Full, compare the 7079 actual source inputs before/after, verify inherited inputs and old evidence, then publish final results. Static Standards PASS and static Spec PASS cannot replace any of those checks.

Summary: Standards 0 findings; Spec 0 findings. Neither axis has a worst issue.

Primary completion readback after both independent reviews: final-full-v2 exited 0 with all 45 steps PASS. All 7079 source records and the full before/after manifest hash are identical. Inherited18 content/mode/link/uid/gid and historical42 objects are unchanged; inherited tracked diff is byte-identical. These are primary-agent checks, not tests run by either reviewer.
