# P12-05 server-only Krill Models preflight

| Field | Result |
|---|---|
| Purpose | Separate a server egress/selected-credential failure from the isolated Staging Messages protocol failure before considering a decoder repair. |
| Request boundary | Exactly one direct HTTPS `GET /models`, no request body, no proxy, no redirect, and the already verified non-secret Codex-compatible User-Agent. The selected Bearer arrived over stdin only. |
| Result | `terminal_state=COMPLETED`; `status_class=2XX`; `content_type_class=json`; bounded `data_list_count_9`. |
| Persistence | The response body, endpoint, Bearer, OAuth material, model names, account identity, and token fingerprints were not retained. The ephemeral classifier deleted itself and its temporary header file. |
| Staging / incumbent effect | No Staging database, route, Client Key, encrypted Credential envelope, listener, service configuration, or incumbent CPA state changed. |
| Interpretation | The current CC Switch-selected Krill base URL and Bearer work from the server. The earlier Staging Messages `5xx` therefore remains a request-shape or decoder-boundary investigation, not a reason to refresh or re-login. |

## Scope limit

This check is a single diagnostic request, not the authenticated loopback-Staging Models
acceptance and not a substitute for the ordered P12-05 sequence.  It neither permits Tool,
Explain, P12-06, nor public exposure.
