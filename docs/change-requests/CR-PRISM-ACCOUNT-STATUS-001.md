# Secret-free account administration

2026-09-11. Implemented locally for the approved CPAMP functional redesign.

`PATCH /admin/credentials/{credential_id}/status` (`updateCredentialStatus`) takes
`{status: "active" | "disabled", credential_revision: nonnegative integer}`. Existing management
authentication, X-Config-Version and If-Match apply. This is draft-only. The response is the
existing Credential projection with the next configuration ETag and credential revision.

The service retains the exact ciphertext, identity, owner and kind; no plaintext is opened and
no secret input is accepted. It checks the credential revision and performs the update and
`credential_status_updated` audit within the guarded draft transaction. Config and credential
conflicts are not replayed. Bounded blocking admission keeps storage work off the HTTP executor.

Prism uses the complete credential inventory as its default account list. Runtime bindings,
cooldown, recovery and evidence remain available under the account runtime view. Codex OAuth
reauthorization is directly available for the actual oauth_json type. New bearer credentials
are visible before binding; a same-origin provider deep link opens their owning resource panel.
Current status changes say saved to the pending configuration, not applied to the running gateway.
Initial OAuth, bulk import, account profile edits and runtime configuration adoption remain open.

Validation: SQLite/HTTP test proves ciphertext preservation and rejection of a stale credential
revision. Browser regression creates an unbound bearer account, discovers it through server search,
disables it without secret input and follows its provider link. Existing runtime/evidence and
three-viewport destination checks remain covered. No production changes.

Final local checks: Clippy, TypeScript and the 115-operation authority/four-file double-build
gate passed. Four new browser checks passed; existing nine runtime/evidence/resource-name and
three-size workspace checks also passed. Actual screenshots at 1440×900 and 390×844 were
inspected; mobile now uses cards with directly visible actions instead of a wide table.
