# Complete managed account and endpoint inventory

2026-09-11. Backend contract finalized for the authorized functional redesign; implementation and verification in progress.

Adds GET `/admin/credentials` (`listManagedCredentials`) and GET `/admin/endpoints`
(`listManagedEndpoints`). Both use existing management authentication and required X-Config-Version.
Optional `upstream_id`, literal `q`, `limit` 1–100 and opaque `cursor` select a bounded page.
The response is `{config_version, revision, observation_version, items, next_cursor}` with ETag.
Credential items contain the existing redacted Credential plus binding_count; endpoints reuse Endpoint.

Queries include unbound, disabled and draft records. They select no ciphertext or plaintext credential
material and do not invoke Providers. SQL applies filtering, exclusive ID and limit. Reads use an
independent read-only connection under existing bounded blocking admission, not the mutation mutex.

Cursor identity includes resource type, version, filters, graph revision and resource audit sequence.
This also rejects continuation after OAuth rotation on an active graph with an unchanged configuration
revision. On 409 discard the paginated set and reload; do not silently append or replay writes.
Case folding follows SQLite lower(); q searches IDs, owner and kind/adapter, not decrypted email.

This implements part of the inventory need in CR-FE-001. It does not claim OAuth onboarding,
credential import, automatic configuration apply or runtime account health as completed. Those are
separate tasks in the functional redesign plan. Authoritative schemas are in management-v1.json;
frontend contract/client must be regenerated using sync-contract.
