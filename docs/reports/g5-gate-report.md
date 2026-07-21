# G5 Anthropic/Claude Code phase gate report

| Field | Value |
|---|---|
| Plan | `v1.9` |
| Gate | `G5` |
| Date | `2026-07-22` |
| Verification branch | `codex/p5-anthropic` |
| Local closeout target | `33a946a65708ad792f787467f94bf08658b4af81`; annotated tag `phase-p5-complete` |
| Local result | `PASS` |
| Remote delivery status | `PASS`; [GitHub Actions 29864915127](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29864915127) accepted the tag SHA with Fast, Full supply-chain, and Required |

## Conclusion

P5-01 through P5-08 have independently committed local evidence and are now `DONE`: the
`phase-p5-complete` tag Gate accepted closeout SHA `33a946a65708ad792f787467f94bf08658b4af81`.
The Phase review confirms the intended P5 boundary: Anthropic
Messages decoding/encoding, exact count capability, Tool terminality, transform admission,
Endpoint-format isolation, explicit semantic projection, and Claude Code client compatibility all
remain bounded by synthetic fixtures or a loopback-only client. No P5 task implements Provider
transport, reads real Provider credentials, sends real Provider traffic, changes a database, or
starts P6.

The acceptance action was deliberately singular. The Phase branch was pushed for reachability,
then annotated tag `phase-p5-complete` triggered exactly one classified code delivery run. GitHub
accepted the tag SHA: Fast passed in 3m01s, Full supply-chain passed in 43s, and Required passed in
4s. The ordinary branch push had no automatic CI trigger and is not counted as delivery evidence.

## G5 conditions and local evidence

| Condition | Evidence | Result |
|---|---|---|
| Claude Code normal dialogue, one Tool, parallel Tools, and Plan Mode work through the gateway boundary | `P5_07_CLAUDE_CODE_BIN=/Users/developer/.local/bin/claude cargo test --locked -p gateway-http-actix --test p5_07_claude_code_bare_e2e -- --ignored --exact local_claude_code_bare_covers_normal_tool_parallel_tool_and_plan_mode` passed with installed Claude Code `2.1.214` in 6.74 seconds. The harness binds only `127.0.0.1`, clears its environment, uses a synthetic key/model, and scripts only fixed `printf` Tools. | PASS |
| Empty Tool input is `{}` and non-empty incomplete JSON fails closed | P5-03's fixed Tool properties plus P5-08 `truncated_tool_is_rejected_without_a_successful_anthropic_termination` passed. The latter permits no `message_delta`, `message_stop`, or completed response from a partial Tool prefix. | PASS |
| Responses/Anthropic cross-protocol routing occurs only after semantic admission | P5-04's pure `analyze_protocol_transform` matrix rejects opaque/extension/history/Thinking/cache and missing-capability cases before any endpoint or credential path. P5-05 consumes only exact native Endpoint format before Health, Quota, or lease admission. | PASS |
| One protocol failure on a shared Upstream does not contaminate another Endpoint | P5-05 controlled same-Upstream E2E proves an OpenAI Responses connection failure cools only its exact Endpoint; the Anthropic Endpoint still selects and succeeds. | PASS |
| Anthropic SSE terminal, Usage, Thinking/cache, explicit stop, and safe errors retain target semantics | P5-06 fixture and HTTP/SSE tests, together with P5-03 Tool-state properties, require explicit stop reason, exact Usage/cache projection, safe error mapping, and protocol-valid terminal output. | PASS |
| Unknown request data and malformed/cancelled streams fail safely | P5-08's fixed synthetic corpus, two fixed-seed 256-case protocol suites, and 128-case cancellation suite passed. The local rerun completed all four `protocol-anthropic` tests and the `gateway-stream` cancellation test. | PASS |

## Phase review

- Reviewed P5-01 through P5-08 reports, their indexed ADR/Contract entries, the P5 plan matrix,
  and the traceability links. Every normal P5 task is independently committed and is `DONE` after
  the tag Gate; P5-00 is the already accepted delivery-infrastructure
  exception and does not claim Anthropic feature behavior.
- Reviewed the Phase diff and crate boundaries. `protocol-anthropic` remains protocol-pure;
  `gateway-stream`'s new `proptest` edge is a documented development dependency only; no HTTP,
  Provider, routing, persistence, credential, or public runtime dependency was introduced by
  P5-08.
- Reviewed the live-client boundary rather than treating a successful CLI exit as sufficient:
  the P5-07 harness is loopback-only, clears ambient configuration, uses synthetic values, and
  records only aggregate counters. It is client compatibility evidence, not Provider or deployment
  evidence.
- The final local `./scripts/check.sh full` passed in 15.94 seconds. It included shell/CI/plan
  guards, format, workspace Clippy/tests, source and crate-boundary policy, documentation links,
  tracked Secret scan, pinned dependency policy, and RustSec audit. Its intentional ignored
  real-provider/diagnostic targets remained ignored.

## Delivery record and boundary

The P5-00 delivery contract requires exactly one normal remote event. Its tag run restored the
default-ref key `quality-tools-Linux-rust-1.97.1-6c77927386864b14a70fbd4b3993fbca77817a4e79756de7e1630982c2584144`
as a cache hit, verified `cargo-deny 0.20.2` and `cargo-audit 0.22.2` without reinstalling them,
and passed Fast, Full supply-chain, and Required. The cache hit accelerated tool setup but did not
replace any quality or supply-chain check.

No pull request was opened for this closeout because it would add an extra CI event contrary to the
Phase-level delivery contract. P6 is eligible to start only on a later explicit instruction; this
report does not itself start P6, merge the Phase branch, deploy, change a server, or send a real
Provider request.
