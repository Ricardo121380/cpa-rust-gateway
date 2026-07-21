# BC-E2E-003 Claude Code loopback `--bare` compatibility

| Field | Value |
|---|---|
| Contract | `BC-E2E-003` |
| Task | `P5-07` |
| ADR | [ADR-0040](../adr/ADR-0040-claude-code-loopback-client-boundary.md) |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Domain | Explicit local Claude Code client compatibility at the Anthropic HTTP boundary |

## Preconditions and containment

1. The ignored test is enabled only with an explicit executable path in `P5_07_CLAUDE_CODE_BIN`.
2. The gateway binds one listener on `127.0.0.1` and accepts one synthetic, enabled Client Key and
   one synthetic public model. No repository configuration, `.env`, ambient API-key variable, real
   Credential, Endpoint, or Provider executor is loaded.
3. The child process starts with `env_clear`; it receives a minimal system `PATH`, temporary `HOME`,
   loopback Anthropic base URL, synthetic API key/model values, and `NO_PROXY` for loopback only.
   The harness does not claim to impose a machine-wide egress sandbox; it provides no configured
   external gateway target or usable real credential.
4. The scripted executor recognizes only fixed test markers and emits deterministic Canonical
   text/Tool events. Tool arguments are fixed local `printf` commands; no fixture can read files,
   make a network request, or interpolate client input into a command.

## Required behavior

| Concern | Required behavior |
|---|---|
| Base probe | `HEAD /` returns `200 OK` with an empty body before authentication. It exposes no model, route, key, snapshot, or Provider status. |
| Client-key admission | An authenticated route accepts exactly one `Authorization: Bearer <key>` or one `x-api-key: <key>`. Duplicate values, mixed schemes, blanks, whitespace, malformed Bearer syntax, and invalid keys fail before body decode or executor work. |
| Client request shape | The real CLI may issue `HEAD /`, `POST /v1/messages?beta=true`, a title-generation Messages request, and retained forward-compatible fields. The gateway uses the established P5 codec/semantic rules rather than treating those observations as a permissive body bypass. |
| Normal dialogue | A `--bare --print --output-format=json` invocation receives the fixed normal marker and returns successful structured CLI output. |
| One Tool | A single assistant `Bash` Tool call reaches the fixed local command, returns one Tool result, and the follow-up Messages request returns the expected marker. |
| Parallel Tools | Two logical Tool calls are issued before either completion; the follow-up carries exactly two Tool results and returns the expected marker. |
| Plan Mode | `--permission-mode plan` completes without a Tool call and returns the expected Plan marker. |
| Trace privacy | The harness retains only aggregate scenario counters in memory. It persists and reports neither raw bodies nor values of Authorization, `x-api-key`, model, URL, Tool output, or Client Key. |

## Failure semantics

| Condition | Result |
|---|---|
| Missing/invalid binary path | The ignored test stops before starting a listener or CLI process. |
| CLI non-zero exit, JSON error result, or absent expected marker | The test fails with a fixed safe local diagnostic. |
| CLI run exceeds 20 seconds | The child is killed and the test fails with a bounded timeout error. |
| Incorrect Tool-result cardinality or unexpected request marker | The scripted executor returns a safe stream protocol error; it does not continue with an unrecognized request. |
| Header ambiguity or invalid key | The HTTP route returns its established safe unauthorized envelope before decode/Provider execution. |
| Loopback server join failure | Test teardown fails; no later scenario is treated as passing. |

## Corresponding tests

- `base_url_head_probe_is_public_and_empty`
- `responses_rejects_ambiguous_or_invalid_client_key_inputs_before_decode_or_provider_execution`
- `messages_accepts_anthropic_x_api_key_without_bearer`
- `local_claude_code_bare_covers_normal_tool_parallel_tool_and_plan_mode` (`#[ignore]`, explicit
  local binary only)
