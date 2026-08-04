# P12-10F grok2api memory-stream migration adapter

Status: `DONE`

## Outcome

CPAR now has a bounded, fail-closed NDJSON migration adapter for Build, Web and Console accounts
plus the frozen grok2api Web-to-Build and Web-to-Console relationships. The adapter accepts only a
`BufRead` pipe: it has no file path, source database, process-spawn or network API. Plaintext is
validated in bounded memory, zeroized after conversion and immediately authenticated-encrypted by
the existing native account transaction.

The source contract was checked against grok2api `v3.0.10` revision
`c27f0545197b3edf41d5deedcc2c3c3597887766`: its `provider_accounts` and
`account_credentials` rows isolate `grok_build`, `grok_web` and `grok_console`, while
`account_provider_links` and `web_console_account_links` express the two supported link kinds. Its
existing `/accounts/export?provider=...` boundary already decrypts each Provider only in the source
process; the later controlled operator helper must normalize those three different export documents
directly into this pipe rather than write an intermediate plaintext file.

## Frozen pipe contract

Each non-empty line is one strict JSON object and is bounded to 1 MiB. A transaction accepts at most
8192 records and 64 MiB total. Account records carry an opaque source reference, provider identity
key, canonical provider credential payload, active/reauth state and bounded scheduler deadlines.
Link records may only express Web-to-Build `web_build` or Web-to-Console `web_console` relations.

- Build credential payloads must already be one supported durable absolute-expiry OAuth document;
- Web payloads must be a strict CPAR SSO session document with an evidence-backed absolute expiry
  and scoped secure Cookies;
- Console payloads are the bounded canonical envelope emitted by the source helper: `sso_token`
  plus the source-observed `probe_model`; CPAR validates the observed model and extracts only the
  SSO token for the cookie, while the controlled route/request supplies its separately approved
  upstream model;
- unknown fields/providers/statuses, duplicate source references, unresolved/invalid links,
  unsupported credential shapes and any bounds violation fail closed;
- any rejected record prevents the database transaction; receipts contain counts only.

The source helper must not invent a Web expiry when grok2api's export lacks one. P12-10G must derive
that field from controlled live evidence or reject the account; a guessed lifetime is not migration
success.

## Atomicity and rollback

The native account store now resolves relation indices and inserts accounts, encrypted credentials,
links and the batch audit row in one SQLite transaction. A changed existing credential aborts the
entire transaction, including newly encountered accounts and links. Exact re-execution under a new
batch ID reports existing accounts as unchanged. Rolling back a newly created batch removes its
accounts and cascades its links; grok2api remains untouched throughout.

## Acceptance evidence

Five dedicated tests cover:

- all three providers and both link kinds;
- absence of identity/token plaintext in the CPAR SQLite file;
- exact rerun idempotency and value-free source/accepted/rejected/link/create/unchanged counts;
- malformed, unknown, duplicate and unresolved records causing zero imports;
- oversized input rejection;
- changed-existing conflict leaving no new account, link or batch row;
- batch rollback cascading all imported links.

The complete `provider-grok` suite and strict all-target Clippy pass. No live account, server export,
external request, production graph, grok2api process or CC Switch configuration was read or changed.

## Next boundary

P12-10G creates a root-only, ephemeral source-to-import pipeline for a controlled live subset. It
must independently count the source export, reject accounts without evidence-backed runtime fields,
verify direct CPAR account attribution/quota/cooldown and all three public protocols, and fully roll
back CPAR on the first failure. The original grok2api pool remains unchanged and available.
