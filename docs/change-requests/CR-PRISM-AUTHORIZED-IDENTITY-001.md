# Capture account identity during authorization

User correction (2026-09-12): identity must be acquired from authorization/import, not manually
backfilled or inferred from an import label. This supersedes the manual-supplementation question.

## Identified defects and changes

Grok Build requested openid/profile/email but discarded id_token, and device grants persist a
compact credential while the initial management identity reader only decoded JSON. The provider
now extracts allowlisted human fields when parsing the grant/import, keeps identity through
refresh, and persists it inside the same AEAD-protected credential. No access token or id_token
is serialized to the management response. Legacy compact v1 remains readable; a credential with
identity writes compact v2. Identity-free credentials still write v1.

Missing grant identity triggers one bounded read of `https://auth.x.ai/oauth2/userinfo` using the
existing OAuth access token. The fixed endpoint is advertised by the issuer's discovery document;
redirects are rejected, timeout is 10 seconds, body maximum 64 KiB, and returned sub must match
access-token sub. No additional scopes or automatic request retries. Provider errors preserve a
valid grant and report identity_state=unavailable; an empty profile is not_provided.

Native authorization start accepts an optional trace name. Start/poll/cancel views add nullable
identity and identity_state (pending/observed/not_provided/unavailable). Completion shows the
provider identity. Labels and source batches are no longer fallbacks for human identity.
Build imports and legacy runtime refresh share the acquisition path; Codex already preserves exported email/id_token and
refresh metadata. API Key and bare SSO formats cannot imply a person where the source exposes none.
Other providers' first OAuth remain separately incomplete, not fabricated here.

## Persistence / rollout

Database schema remains 24; existing credential ciphertext/revision/audit boundaries apply. v2
credential content is understood by the new binary, not the previous deployed reader. A future
release must retain the old backup and explicitly plan credential-format rollback; copying the
old binary over newly written v2 credentials alone is not a valid rollback. No production rollout
or production state mutation is part of this implementation turn.

Discovery evidence: https://auth.x.ai/.well-known/openid-configuration (checked 2026-09-12).
Profile claims are display data and never become authorization or entitlement grants.
