# P8-07 Grok Official authorized one-probe report

| Field | Value |
|---|---|
| Plan version | `v1.41` |
| Task | `P8-07` |
| Date | `2026-07-23` |
| Branch | `codex/p8-official` |
| Status | `DEFERRED` — local safety harness is complete; no Official API key is available, so this E2E moves to the final external-authentication package with P7-09. |
| Scope / budget | `S`; harness and documentation only. No xAI request, credential source, account, server, route, proxy/TUN setting, or production configuration was read or changed. |
| References | §20.1 Provider Gate E2E; [ADR-0060](../adr/ADR-0060-grok-official-authorized-one-probe.md); [BC-E2E-004](../contracts/BC-E2E-004-grok-official-authorized-one-probe.md) |

## Delivered safety boundary

`p8_07_authorized_official_probe` is ignored by default. Its ordinary tests exercise only
in-memory synthetic values and prove that missing authorization and a request cap other than one
stop before preparation. Complete synthetic configuration can construct the fixed native adapter
without DNS or transport.

The one live test has a single native adapter `execute` call. It fixes the Official Responses
endpoint, direct DNS-pinned egress, redirect denial, one pooled connection, finite timeouts, one
mode, and a one-request cap. It does not read generic Provider configuration, catalog entries,
OAuth caches, server files, proxy/TUN settings, or fallback credentials. It never retries,
refreshes, rotates, fails over, or selects a model.

Its console output is limited to an operator-provided opaque label, `non_streaming` or `sse`, and
`started`, `pass`, or one fixed stopped category. It cannot print the API key, model, headers,
endpoint variant, request/response body, account data, or generated text.

## Local verification and review

| Command / review | Result |
|---|---|
| `cargo test --locked -p provider-grok --test p8_07_authorized_official_probe` | PASS; 3 default zero-network tests pass and 1 live test remains ignored. |
| `cargo fmt --all -- --check` | PASS. |
| `cargo clippy --locked -p provider-grok --test p8_07_authorized_official_probe -- -D warnings` | PASS. |
| Focused review | PASS: no ambient proxy or credential discovery, no unredacted values, exactly one external execution path, no retry/failover, and ignored/default tests remain zero-network. |

## Live stop boundary

No xAI Official request has been sent. After the remaining local phases are complete, a later live
invocation requires a separately supplied P8 Official API key and model plus explicit authorization
for one opaque target and one mode. Its only accepted result is Canonical `ResponseStart`, text,
and clean `ResponseEnd`. That P8 result and P7's Kiro OAuth validation remain separate proofs even
though `CR-P8-DEFER-001` schedules them in the same final external-authentication package.
