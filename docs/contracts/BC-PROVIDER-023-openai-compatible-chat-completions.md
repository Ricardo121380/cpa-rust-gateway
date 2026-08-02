# BC-PROVIDER-023 OpenAI-compatible Chat Completions adapter

| Field | Value |
|---|---|
| Contract | `BC-PROVIDER-023` |
| Task | `P12-08C` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Reference | CLIProxyAPI `v7.2.101` native OpenAI translator |

## Native request behavior

`openai/chat-completions` is a third exact `ApiFormat`, served only by
`openai-compatible.chat-completions`. Publication rejects unknown spellings and an adapter paired
with another format. The P12 runtime composes the same closed pair and fails the complete Version
when no binding exists.

For a native Chat caller, the authenticated HTTP boundary retains the byte body only after the
strict Chat decoder accepts it. The outbound adapter revalidates that body, requires the trusted
ingress mode to agree, and replaces only `model` with the Snapshot-selected upstream model. This
ports the incumbent CPA's native behavior: valid provider-specific Chat fields stay present rather
than being unnecessarily rebuilt through Canonical. The retained body is never logged or rendered
by `Debug`.

Canonical reconstruction exists as a separate fail-closed builder for P12-08D. It rejects
Thinking, cache controls, foreign extension namespaces, reserved collisions and wire shapes Chat
cannot represent without loss.

## Response and transport behavior

- The request reaches the shared DNS-pinned client only after exact-target egress admission. A
  different admitted URL is rejected before the credential/body enters transport.
- JSON and SSE responses require one choice, assistant output, supported Tool calls, checked Usage
  arithmetic and a closed finish-reason mapping (`stop`, `length`, `tool_calls`).
- SSE framing is independent of transport chunking. Tool argument fragments assemble to valid JSON;
  normal completion requires finish, optional usage-only chunk, and exactly one `[DONE]`.
- Unknown semantic response fields, nonzero unsupported token details, invalid Tool lifecycle,
  missing `[DONE]`, oversized residue and malformed JSON fail closed before client delivery.
- Credentials, endpoint, model and request/response content remain redacted from diagnostics.

## Reference-port boundary

CPA `v7.2.101` is a behavior and test source, not a dependency. CPAR keeps its stricter Secret,
DNS-pinned egress, bounded buffering, Canonical lifecycle, retry/FSE and publication rules. Known CPA
defects are not compatibility requirements; in particular, an incomplete SSE lifecycle cannot be
reported as success.
