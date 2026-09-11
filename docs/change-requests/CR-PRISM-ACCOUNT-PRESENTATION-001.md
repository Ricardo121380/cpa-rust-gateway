# Account identity and connection presentation

The user requests six independent account families and human identities instead of phase IDs,
digests or import-batch labels. This changes presentation, not configuration identifiers or routing.

`listManagedCredentials` now includes `identity`, `category`, `provider` and bounded `connections`.
`listNativeAccounts` includes the same nullable identity object. No secrets are serialized. Identity
is projected from allowlisted fields of stored credential JSON/JWT on the server; JWT claims are
unverified display evidence and never affect authorization. Opaque subjects are never usernames.

Ordinary projection runs in the existing bounded blocking admission and one read-only SQLite
snapshot; native projection checks its generation again after decryption. Undecryptable or absent
identity is null, not a fabricated email. No external provider lookup, migration or history edit.

Connection previews show actual protocol, hostname and administrative enablement. Counts include
all configured bindings and do not imply active runtime selection or health. Native Grok accounts
belong to the channel pool; the UI must not invent per-account endpoint binding counts.

Technical-ID server search remains compatible. The unified account UI labels its identity search
as searching loaded accounts and keeps pagination explicit; partial counts never claim totals.
