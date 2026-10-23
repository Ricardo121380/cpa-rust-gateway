# BC-PROTOCOL-016: Claude Code D2 restricted support

| Field | Value |
| --- | --- |
| Contract | BC-PROTOCOL-016 |
| Task | #64 D2 controlled acceptance; #58/#66 selected first-release Claude configuration |
| Decision | [ADR-0122 experiment](../adr/ADR-0122-isolated-claude-restricted-candidate.md); [ADR-0123 product adoption](../adr/ADR-0123-first-release-claude-d2-restricted-support.md) |

## Adopted first-release scope (2026-10-23)

D2 is ADOPTED for the first release. The evidenced client is unmodified Claude Code 2.1.302,
entry `/Users/huangrui/.local/bin/claude`, resolved binary
`/Users/huangrui/.local/share/claude/versions/2.1.302`, SHA-256
`5262ce9de86243fed0d21edc39b271555d56780d0112e2b194439fe1c6662d37`.
Use the exact local Messages profile and all four values:

```sh
DISABLE_PROMPT_CACHING=1
CLAUDE_CODE_DISABLE_THINKING=1
CLAUDE_CODE_MAX_RETRIES=0
CLAUDE_CODE_DISABLE_NONSTREAMING_FALLBACK=1
```

[Local configuration and limits](../support/grok-build-messages.md) and the
[candidate inventory](../support/first-release-candidates.md) distinguish product selection
from controlled evidence, binary/OCI lifecycle, fixed-service qualification and real acceptance.
Codex, Claude D2 and Pi must all complete the agreed Grok Build gates before first-release READY.
This contract adds no production native-field qualification or controlled-test-support delivery.

## Source and candidate boundary

Primary sources checked 2026-10-22: [environment variables](https://code.claude.com/docs/en/env-vars),
[gateway recovery](https://code.claude.com/docs/en/llm-gateway-protocol#automatic-retry-and-error-forwarding),
and [model configuration](https://code.claude.com/docs/en/model-config).
The two candidate variables disable prompt caching and omit the thinking request parameter.
Actual captured requests decide whether all corresponding wire fields disappeared. Thinking
omission is not a promise that the model stops reasoning. Capability declarations documented for
cloud-provider modes do not apply to ANTHROPIC_BASE_URL. auto selects default effort, not absence.
Current rolling documentation describes recovery, not a minimum-version or exact-matcher proof.

The evaluated D2 configuration also sets the official CLAUDE_CODE_MAX_RETRIES=0 to disable
ordinary API retries. With the default retry policy, the actual client retried a permanent
Messages stream failure; that failed observation remains evidence. This bound must itself be
captured and reverified alongside effort capability recovery. It changes client retry behavior,
not request semantics, error wording, gateway retries or Provider classification.
MAX_RETRIES=0 alone still allowed one non-streaming fallback after the stream failure. That
second failed observation is also retained. Set the separately documented
CLAUDE_CODE_DISABLE_NONSTREAMING_FALLBACK=1; errors then propagate to the bounded retry layer.
All four D2 variables are part of the evaluated configuration, including normal/cancel runs.

The unmodified installed Claude, its identity/hash/mode/symlink, isolated HOME/config/workdir,
grok-4.5 and real Build assembly are fixed per run. D2 does not promise adaptive/display, overall
effort or block cache controls. Those original #58/#64 requirements remain open follow-up work,
outside the selected first-release configuration, without being removed or completed. Original
Claude remains a separate rejection branch; #58/#64 and their dependencies remain open.
ADR-0123 adopts only the evidenced D2 configuration; fixed Build activation remains separate.

## Local effort rejection and negotiation

Only a closed output_config object containing the sole string member effort is eligible for
the specific diagnostic. Duplicate names, extra members and unknown fields retain rejection.
Complete body/history, controls and effective capabilities must pass their independent checks.
Every applicable candidate, including candidates with no active credential binding, must reject
for this same reason. Empty/exact-model-mismatched sets, missing runtime/adapter evidence, mixed
reasons or any potentially compatible candidate cannot produce this field-specific claim.

Preserve actual auth, stale runtime, unavailable service, continuation and already-sent errors.
No request identity, model, URL, user_id, tool name or supplied value enters the fixed message:

```text
output_config.effort: Extra inputs are not permitted by the current gateway conversion; no upstream request was sent
```

Return HTTP400 with the existing standard Messages type=error and
error.type=invalid_request_error envelope and public Request-ID header. The documented client
mechanism recognizes the field name and rejection phrase. The message explicitly attributes
the rejection to CPAR's conversion; it is not a fabricated Provider response or native error.

The original Request remains rejected: receiver=0, lease=0 and no Attempt/Usage/ledger. Existing
valid M4 capture commits before that rejection and remains authorized Request-only evidence.
Only the actual client can issue the next request without effort. That is a new Request with
independent authentication and actual execution evidence. CPAR never strips or retries it.
If this current client does not negotiate correctly, retain RED and stop dependent positives.

## Actual-client acceptance

The successful branch must receive Read/Bash calls, execute both tools itself (seed.txt=41,
result.txt=42), send their real results and exact associations, continue history and answer a
follow-up. CLI exit=0 and every completed native result has subtype=success and is_error=false.
Per-request ingress, independent receive, actual Build model/account/Attempt, known Usage and
ledger joins must agree; negotiation/background/preflight requests have separate counts.

Recheck complete tool history each round. Preserve absent thinking/signature; hidden Grok summary
never enters public content/history and is fully encrypted/read back through #63's authorized
reader. Read M4 raw bytes unchanged with its source/owner/real Attempt (or Request-only).
Failure retains known Usage and one failed terminal without resend or tool completion. Cancel
only after an actual public delta and lease=1, using a current supported client interruption.
The same PID stays alive through independent upstream close, lease=0 and durable public Cancelled;
only then may cleanup begin. Killing the process or peer alone is not cancellation evidence.
Unknown Usage remains unknown, and #61 cost_in_usd_ticks remains NOT_ADMITTED.

## Corresponding tests

- `public_build_messages_effort_rejection_is_specific_and_preserves_request_only`

- `public_build_messages_effort_diagnostic_preserves_independent_rejections`
- `public_build_messages_missing_runtime_is_not_an_effort_rejection`
- `actual_claude_messages_d2_qualification`
