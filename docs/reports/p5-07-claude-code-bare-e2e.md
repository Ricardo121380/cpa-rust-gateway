# P5-07 Claude Code `--bare` loopback E2E report

| Field | Value |
|---|---|
| Plan version | `v1.9` |
| Task | `P5-07` |
| Date | `2026-07-22` |
| Branch | `codex/p5-anthropic` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Scope / budget | `M`; Claude Code compatibility admission and isolated local E2E only |
| Execution channel | Current default model, `medium`; fallback: Luna is unavailable in this execution surface. No subagent was used because the change affects the public authentication boundary and requires one accountable review. |
| References | Matrix `A07`, `B09-B16`, `B25-B28`; [ADR-0040](../adr/ADR-0040-claude-code-loopback-client-boundary.md); [BC-E2E-003](../contracts/BC-E2E-003-claude-code-loopback-bare-compatibility.md) |

## Delivered boundary

The controlled local Claude Code `2.1.214` probe established that the client first sends `HEAD /`
and subsequently sends Anthropic Messages requests using `x-api-key`. The shared HTTP admission
now accepts precisely one strict Bearer or `x-api-key` presentation, rejects every duplicate or
mixed form before decoding, and preserves the existing authenticator as the only key verifier.
`HEAD /` is deliberately body-free and unauthenticated; it is a reachability probe, not discovery.

The ignored `p5_07_claude_code_bare_e2e` target starts a one-worker Actix server only on loopback,
clears the client environment, and uses a synthetic key/model plus fixed local `printf` Tools. It
does not load real configuration or execute a Provider request. It retains only aggregate counters
in memory; no raw request body, response body, header, API key, URL, model, or Tool output is
written to a report or test artifact.

## Local verification

| Command / review | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --locked -p gateway-http-actix` | PASS; 28 HTTP unit tests, P3 local integration coverage, and ordinary guard tests passed; explicit live/diagnostic targets remained ignored. |
| `P5_07_CLAUDE_CODE_BIN=/Users/developer/.local/bin/claude cargo test --locked -p gateway-http-actix --test p5_07_claude_code_bare_e2e -- --ignored --exact local_claude_code_bare_covers_normal_tool_parallel_tool_and_plan_mode` | PASS; Claude Code `2.1.214` completed normal dialogue, one Tool, parallel Tools, and Plan Mode in 6.99 seconds. The safe trace asserted at least one title request, one request for each scenario, and exactly three received Tool results. |
| `cargo clippy --locked -p gateway-http-actix --all-targets --all-features -- -D warnings` | PASS |
| `./scripts/check.sh full` | PASS; public HTTP/authentication change received the required local full Gate. |
| `git diff --check` and staged Secret scan | PASS; review found no whitespace issue or credential-like artifact. |

## Review conclusion

The review followed the value flow rather than merely accepting a successful CLI run. `HEAD /` has
no response body and no state lookup. For every authenticated route, both header collections are
inspected before choosing a value: each must be singular, exactly one scheme must be present, and
the supplied text must be non-empty and whitespace-free before the existing authenticator sees it.
The error path is therefore unable to decode an attacker-controlled body or invoke an executor.

The E2E has a narrow trust boundary: it proves the actual installed client against a local Actix
listener, but it is not a real Provider test, a deployment test, or a general operating-system
network sandbox. Fixed Tool arguments and the marker-only executor make the observed Tool and Plan
flows reproducible without turning the test into arbitrary code execution.

## Rollback and next task

Reverting P5-07 removes the loopback readiness route, `x-api-key` presentation support, and the
ignored client harness. No data, schema, credential, service, or external request requires
cleanup. P5-08 is now the sole `IN_PROGRESS` Task; it owns fixed-corpus fuzz/property coverage for
unknown fields, malformed streams, truncated Tools, and cancellation. P5 still has one G5
Phase-level remote Delivery Gate after local closeout.
