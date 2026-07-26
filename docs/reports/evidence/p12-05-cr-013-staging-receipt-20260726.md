# P12-05 CR-013 isolated Staging receipt

| Field | Result |
|---|---|
| Scope | One fresh loopback-only Models plus non-streaming Anthropic Messages validation using the independently verified CR-013 artifact; no Tool, Explain, direct probe, P12-06, or public exposure. |
| Artifact | Revision `49f8c0f3eb6326d3f2ed6cc612ec8ffd10915938`; Gateway SHA-256 `e6e13f3a3e69eb7d0ede11815c9b526bf8d6e33a96c5343307b8faf20eb260f4`; manifest SHA-256 `367eb2ff4f0dea16b3f69ce6a780e7f504cc233865cb9e73c4401dadf49388d5`. |
| Local credential selection | The memory-only helper confirmed the current Krill/Codex profile has all OAuth fields, selected one of its two configured HTTPS endpoints, and used a distinct effective Bearer. No value was rendered, persisted locally, passed as a command argument or environment variable, or copied to Git. |
| Server staging | Exact Linux binary hash and `gateway serve --help` passed before the current link changed. The temporary root-only harness hash and shell syntax also passed. |
| Models | Loopback `GET /v1/models` passed with exactly the temporary public model. |
| Real request budget | `external_messages_request=one`; no retry. |
| Messages | `messages_status_class=2xx`; in-memory response validation confirmed the repaired `end_turn` completion and numeric input/output usage fields. |
| Protected attempt projection | Exactly one value-free row: `attempt_projection=valid`, `attempt_outcome=succeeded`, `attempt_stage=decoder`. |
| Transaction | `transaction_result=completed`, `progress_stage=completed`, `failure_stage=none`, `exit_status=0`. |
| Rollback | `database_preimage_restored=1`, `staging_restart_after_rollback=1`, `incumbent_continuity=1`, `rollback=restored`. |
| Post-review | Temporary harness removed; current release link restored; Staging remains active but disabled at boot; incumbent remains active; listeners are exactly loopback; the Staging database again has no temporary graph resources. |

## Review conclusion

The CR-013 repair is proven through the authorized end-to-end boundary: the same isolated
Messages path that previously failed after decoder completion now returns a valid non-streaming
Anthropic lifecycle without broadening the request, transport, Provider, credential, listener, or
public-traffic surface. The local source regression proves the event order; this receipt proves
the real Staging integration accepted it.

This closes only CR-P12-05-013's one-request Messages validation. P12-05 remains `IN_PROGRESS`:
Tool and Explain were deliberately not run and require an explicit subsequent authorization. P12-06
and every public-exposure step remain pending.
