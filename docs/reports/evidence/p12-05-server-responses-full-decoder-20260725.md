# P12-05 server-only Krill Responses full-decoder classifier

| Field | Result |
|---|---|
| Scope | Exactly one CR-P12-05-009 non-streaming direct HTTPS request, using the same selected in-memory Krill configuration and a root-only receipt-first classifier. |
| Transport result | `terminal_state=COMPLETED`, `request_count=1`, `status_class=2XX`, `content_type_class=json`, and `json_shape=object`. |
| Decoder result | `decoder_gate=accepted_exact_nonstreaming_contract`: the classifier's safe mirror accepted non-empty ID/text, completed status, allowed output items, valid Function-call JSON, Usage/reasoning-token constraints, and the finite Canonical lifecycle preconditions. |
| Persistence | The controlled server receipt is `root:root`, mode `0600`, and contains only the safe categories above. No endpoint, Bearer, OAuth material, model, ID, text, request/response body, header value, or digest was retained. |
| Interpretation | A speculative decoder relaxation is not justified by the CR-008 Staging 502. However, later source review found that this handwritten direct body omitted the P12 builder's fixed input-item `type:"message"`; it is near-shape, not exact P12 outbound-body evidence. |

## Limit

This is not a Staging acceptance and does not prove byte-for-byte identity with the earlier
upstream response.  The missing fixed type member means it does not authorize a decoder change or
additional retry; CR-P12-05-011 separately classifies the exact P12 body.  It authorizes neither
Tool, Explain, P12-06, nor public exposure by itself.
