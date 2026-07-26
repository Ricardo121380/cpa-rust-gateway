# P12-05 CR-014 Tool and Explain receipt

| Field | Result |
|---|---|
| Scope | One fresh loopback-only Tool transaction using the independently verified CR-013 artifact; Explain was conditional on a valid Function Call representation. No Models, Messages, SSE, direct probe, P12-06, or public exposure was included. |
| Artifact | Revision `49f8c0f3eb6326d3f2ed6cc612ec8ffd10915938`; Gateway SHA-256 `e6e13f3a3e69eb7d0ede11815c9b526bf8d6e33a96c5343307b8faf20eb260f4`; manifest SHA-256 `367eb2ff4f0dea16b3f69ce6a780e7f504cc233865cb9e73c4401dadf49388d5`. |
| Credential selection | The memory-only selector reconfirmed all three OAuth fields, one configuration-selected HTTPS endpoint, the active configured model, and a distinct effective Bearer. No value was rendered, persisted locally, placed in a command argument/environment, or copied to Git. |
| Launch correction | An initial local selector stop and a `noexec` temporary-path launch occurred before the harness started. No Staging write or Tool request occurred in that launch; a preimage check confirmed no rollback was needed. The final harness was root-only, hash-checked, and executed from an executable root-owned path. |
| Real request budget | `external_tool_request=one`; no retry. |
| Tool result | `tool_status_class=2xx`; `tool_representation=invalid`. The response body was only held in the root-only process and discarded before receipt creation. |
| Protected stages | `attempt_projection=not_queried`; `explain_result=not_run`; `external_explain_upstream_request=no`. The stop occurred before either conditional stage. |
| Transaction | `transaction_result=hard_stop`, `progress_stage=tool_before_post`, `failure_stage=tool_representation`, `exit_status=1`. |
| Rollback | `database_preimage_restored=1`, `post_rollback_empty_database=pass`, `staging_restart_after_rollback=1`, `incumbent_continuity=1`, `rollback=restored`. |
| Independent post-review | The root-only receipt is mode `0600` and owned by `root:root`; temporary harnesses are absent; current release link was restored; Staging remains active but disabled at boot; listeners are exactly loopback; the control database is again empty; incumbent remains active. |

## Review conclusion

CR-014 proves that the isolated Gateway sent one Tool-shaped request and received a completed HTTP
class, but it does not prove a Tool representation. Because bodies and opaque IDs are intentionally
not retained, `invalid` is not evidence of a Provider defect, model incompatibility, or decoder
bug. Explain did not run, no additional external request was made, and the exact preimage was
restored.

CR-014 is consumed. CR-015 is the separately approved, semantically distinct Tool tuple: it uses
an unambiguous no-effect declaration and a closed, value-free structure classifier. P12-05 remains
`IN_PROGRESS`; P12-06 and public exposure remain prohibited.
