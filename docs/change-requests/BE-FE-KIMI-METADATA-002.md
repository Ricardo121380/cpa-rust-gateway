# BE-FE-KIMI-METADATA-002 — independent metadata failure evidence

Status: implemented locally, pending production release/verification.

Existing `KimiAccountObservation` gains optional nullable `profile_error` and
`quota_error`. Each contains a closed `code`, and only HTTP failures contain a
numeric `status`. Categories: not_observed, egress_denied, transport, timeout,
http, response_too_large, invalid_json, unrecognized_response. No body, URL,
header, token or exception string is returned. Existing fields retain semantics.
`CredentialMetadata.kimi_error` additionally distinguishes busy and timeout at
the management boundary. Unknown usage stays unknown, not zero. One successful
profile never erases the other request's failure. No new endpoint, schema
migration, provider inference or model authorization change.

Authority: `docs/openapi/management-v1.json`; frontend follows sync-contract.
