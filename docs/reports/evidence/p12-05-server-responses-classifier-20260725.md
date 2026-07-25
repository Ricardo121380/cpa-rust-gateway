# P12-05 server-only Krill Responses replacement classifier

| Field | Result |
|---|---|
| Scope | One CR-P12-05-007 replacement for the conservatively closed CR-006 capture failure; non-streaming direct HTTPS only. |
| Request | Current selected CC Switch Krill base URL, Bearer, and configured model entered via stdin; fixed non-sensitive prompt, Responses-compatible body, direct/no-proxy/no-redirect transport, and the verified User-Agent. |
| Safe result | `terminal_state=COMPLETED`, `request_count=1`, `status_class=2XX`, `content_type_class=json`, `json_shape=object`, `decoder_gate=accepted_structural_subset`. |
| Receipt review | The server receipt exists as a regular `root:root` mode `0600` file under the established P12-05 receipt root. It contains only the safe fields above. |
| Interpretation | Server egress, the selected base URL/Bearer, and the visible completed Responses structure are working. This neither proves each Rust decoder/Canonical detail nor replaces the isolated-Staging Messages check. |
| State effect | No Staging configuration, Client Key, service/listener, Caddy, Cloudflare, DNS, public traffic, or incumbent CPA state changed. |

## Review limit

The response body was intentionally not retained.  Therefore this receipt cannot reconstruct the
earlier Staging 5xx or expose its exact gateway error category.  CR-P12-05-008 uses a separate
receipt-enhanced loopback Staging run for that bounded attribution.
