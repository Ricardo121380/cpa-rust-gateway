# P12-05 CR-015 Tool and Explain receipt

| Field | Result |
|---|---|
| Scope | One fresh loopback-only Tool tuple with an explicit no-external-effect declaration; one protected Route Explain only after the Tool and protected attempt succeeded. No Models, Messages, SSE, direct probe, P12-06, or public exposure. |
| Artifact | Revision `49f8c0f3eb6326d3f2ed6cc612ec8ffd10915938`; Gateway SHA-256 `e6e13f3a3e69eb7d0ede11815c9b526bf8d6e33a96c5343307b8faf20eb260f4`; manifest SHA-256 `367eb2ff4f0dea16b3f69ce6a780e7f504cc233865cb9e73c4401dadf49388d5`. |
| Credential selection | The memory-only selector reconfirmed all OAuth fields, one configuration-selected HTTPS endpoint, the active configured model, and a distinct effective Bearer. Values were not rendered, persisted locally, passed as command arguments/environments, or copied to Git. |
| Server preconditions | Empty Staging graph, disabled-at-boot unit, exact loopback listeners, no unit proxy, active incumbent, and original current release all passed before the transaction. The root-only harness was syntax-checked and upload-hash verified. |
| Real request budget | `external_tool_request=one`; no retry. |
| Tool result | `tool_status_class=2xx`; `tool_representation=valid`. The Function was only represented in the response; the Gateway did not execute it. Request and response bodies were discarded in process. |
| Protected attempt projection | `attempt_projection=valid`, `attempt_outcome=succeeded`, `attempt_stage=decoder`; exactly one safe terminal row. |
| Explain | `explain_result=selected`; `explain_attempt_projection=unchanged`; `external_explain_upstream_request=no`. The only configured Candidate was selected and Explain created no upstream attempt. |
| Transaction | `transaction_result=completed`, `progress_stage=completed`, `failure_stage=none`, `exit_status=0`. |
| Rollback | `database_preimage_restored=1`, `post_rollback_empty_database=pass`, `staging_restart_after_rollback=1`, `incumbent_continuity=1`, `rollback=restored`. |
| Independent post-review | Receipt is root-owned mode `0600`; temporary harness is absent; original current link is restored; Staging is active but disabled at boot; listeners are exactly loopback; control database is empty; incumbent remains active. |

## Review conclusion

CR-015 completes the previously missing Tool and Explain evidence without executing a Tool or
leaving any temporary graph. Together with CR-013 and the prior Responses/SSE evidence, P12-05
has covered Models, OpenAI Responses non-streaming/SSE, Anthropic Messages lifecycle, Tool
representation, and side-effect-free Route Explain.

P12-05 is therefore `LOCAL_PASS_PENDING_PHASE_GATE`. This receipt does not begin P12-06, expose a
listener, change public traffic, or alter the incumbent gateway.
