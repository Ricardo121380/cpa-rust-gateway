# P12-08G1 CR-029 terminal-stage receipt

| Field | Value-free result |
|---|---|
| Artifact | Exact CR-028 revision; dual-target build succeeded and ARM64 Sigstore verification passed before deployment |
| Send boundary | One resumed request for failed tuple 7; tuples 1–6 were not resent |
| Public result | Safe failure |
| Durable Attempt | Exactly one; outcome `failed`; final stage `decoder` |
| Queue admission | No Required queue full or closed sink |
| Durable writer | No quarantined Required event or write failure; no pending Required event at observation |
| Rollback | G1 v2 key rejected, predecessor key accepted, service active and disabled at boot, no non-loopback listener |

The prior missing terminal was caused by restarting inside the Route's 15-second Attempt ceiling,
not by a second persistence defect. CR-029 waited for the terminal before rollback and closed the
failure at the decoder boundary. No endpoint, credential, model, request/response body, identifier,
timestamp, token value or fingerprint is retained in this receipt.
