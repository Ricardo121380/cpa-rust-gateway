# Guarded edits of the active configuration

2026-09-11. Backend contract finalized; runtime apply is a separate remaining integration step.

`POST /admin/config-versions/{config_version_id}/fork` (`forkConfigVersion`) requires the
existing management admission, matching X-Config-Version and If-Match. The body contains the new
opaque `id` and `description`. It returns the existing ConfigVersion shape for a complete draft,
without an ETag for the source. This replaces the incorrect assumption that parent_id clones data.

All existing resources, keys and bindings are retained. Credential and compatible proxy ciphertext
is re-sealed server-side with the new version's AAD; plaintext never passes through the client.
The graph is moved through the clone operation rather than duplicating the full sealed graph in memory.

Schema 23 adds `configuration_edit_origins`: source version/revision, resource-audit sequence and
publication/rollback sequence. Origin capture is transactional; creation rechecks it after sealing.
Activation checks the origin inside its write transaction, so concurrent OAuth rotation, graph changes
or lifecycle ABA cannot silently publish old account material. A rejected edit remains a draft.
Legacy drafts with no origin retain existing semantics; archived rollback is unchanged.

The API starts an edit; it does not claim that serving has adopted the draft. The subsequent
runtime-composition integration must be complete before the normal UI labels its operation
“保存并应用”. Migration is local in this implementation batch, not deployed to production.

Interactive OAuth sessions use a length-delimited hash of configuration and credential IDs as
the internal workflow key. Forks preserve public IDs but cannot share, cancel or complete each
other's OAuth session. The response keeps the original public credential ID.

Validation: three control tests (complete graph/resealing, OAuth rotation conflict, lifecycle ABA),
four SQLite-backed HTTP inventory/fork/session tests, existing two OAuth HTTP regressions,
schema migration up/down, focused browser edit flow, TypeScript, Clippy and 114-operation
authority/four-file double-build gate passed.
