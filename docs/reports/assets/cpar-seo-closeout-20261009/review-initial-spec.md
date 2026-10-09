# Initial Spec review

Status: complete. Verdict: **FAIL**, finding 1. Fixed base `27ee36174f54bac059694a0870e452f9ac632ce0`; review HEAD `cb6d5bca6e9aebf6aa081247a907cf22db3205ca`.

Independent read-only reviewer `/root/seo_final_spec`. This record preserves the returned initial verdict and findings, with light formatting cleanup.

**E-03 shared fixture projection still misclassifies Kiro API Key.** Specification `request-spec.md:124` requires “以真实生产契约为准，核正 fixture 的 OAuth 材料分类、账号清单投影及相应操作入口。” `web/prism/src/dev/fixtures.ts:1958` marks only JSON imports as OAuth; a legitimate ksk material is an unmarked bearer. The shared `fixtureCredentialPresentation` at line 719 returns null for every non-OAuth Kiro credential, and both inventory routes use it (lines 2754, 2796). Production `crates/gateway-store/src/account_identity.rs:42` returns api_key for non-JSON ASCII graphic material, consistent with the actual UI API Key evidence.

Static reachable inference: the Kiro API Key fixture is excluded from entry_type=api and enters the account directory; managed inventory regresses from api_key to null. OAuth repair passed, but E-03 production-compatible PASS is incomplete. This does not establish a production classifier defect.

Item verdicts: S-01 PASS (eight original body paragraphs preserved); E-01 PASS_LOCAL, five options/15 fixture branches/five actual reuse flows, Grok inference NOT_RUN; E-02 PASS_HANDLER_AND_FIXTURE, real commit→EOF/original session/exchange=1 and fixture close/return, actual device-loss UI NOT_RUN; E-03 PARTIAL; E-04 PASS_PUBLIC_HTTP, 527=410+117; O-01 PASS_LOCAL, JSON/SSE phase/output/GET/history and strict private envelope; E-05 boundary PASS, approved_rule=null and approved-rule implementation/tests NOT_RUN. No scope creep found.

Coverage: **192/192 files**, 29 text files all hunks, 105 JSON structural checks, 56 decoded gzip archives, two PNGs viewed. All 171 index objects and 56 archive mappings matched. Unread changes: 0. Counts support Full 45/45, Rust 1493/0/12, 494 frontend tests, 18 actual UI/17 fixture scenes. The reviewer did not rerun tests. The official phase guide was independently verified.
