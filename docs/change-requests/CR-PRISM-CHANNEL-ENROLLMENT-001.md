# Channel enrollment: capability directory and validated credential imports

2026-09-11. User confirmed every previously scoped channel needs a proper entry point.
The complete scope remains OpenAI-compatible relays, Anthropic-compatible relays, Codex/ChatGPT,
Claude, Grok Official/Build/Console/Web and Kiro. Compatible third-party brands use their actual
OpenAI/Anthropic protocol family; they are not assigned fictional native OAuth flows.

## Implemented contract

- `GET /admin/account-channels` (`listAccountChannels`): management-authenticated global directory
  of all nine families, credential format, compatible upstream kinds, import availability and
  first-authorization flow/availability. This is enrollment capability, not current credential
  validity, runtime health or model entitlement.
- `POST /admin/upstreams/{upstream_id}/account-import` (`importChannelAccount`): normal draft,
  X-Config-Version and If-Match discipline; bounded single credential with ID, channel and secret.
  The existing Credential projection and new revision are returned, never the submitted material.
  The browser does not fabricate kind. Files remain memory-only and are limited to 64 KiB.
- OpenAI-compatible, Anthropic-compatible and Grok Official accept opaque printable API keys.
  Codex accepts the existing CPA/Sub2API/native parser and re-exports the normalized server-side
  envelope before encryption. Claude and Kiro use their existing strict provider parsers and
  expiry checks. Owner kind must match the selected family. Existing create/audit transaction
  supplies atomic persistence; conflicts are not retried.
- Blocking parser/store work uses the existing four-permit bounded admission. Secret values are
  zeroizing input/material and not returned. The HTTP crate now reuses the already-present Kiro
  and Anthropic parser crates; no new package versions or external service are introduced.

## Remaining required work — not deferred

Native Grok Build/Console/Web use GrokAccountPoolStore, not upstream_credentials. Their import
availability is currently false and requests to the ordinary importer are rejected. They need
native import, complete native inventory and lifecycle wiring, rather than storing tokens in an
unused ordinary record. Initial authorization availability is false for all new-account entries;
legacy Codex reauthorization remains separate. First OAuth, native Grok, channel-correct
reauthorization, multi-account import preview/commit and runtime apply remain required.

## Current verification

Real SQLite/HTTP regression checks the nine-family directory, valid Codex import, redacted
response, unbound visibility and rejection of malformed material/owner mismatch/native-token
misplacement. Parser regression checks API-key, Claude, Kiro, Codex and unsupported-type boundaries.
Five browser checks passed, including the channel selector and file-import path; TypeScript and
Clippy passed. These are synthetic checks, not live authorization or all-channel acceptance.
No production credentials, deployment, historical cleanup or external authorization changed.
