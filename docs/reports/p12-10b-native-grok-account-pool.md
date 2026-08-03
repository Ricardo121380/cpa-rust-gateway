# P12-10B native Grok account pool

Status: `DONE`

## Outcome

CPAR now has a native durable aggregate for Grok Build, Web and Console accounts. This is not a
proxy integration with grok2api: CPAR owns the account records, encrypted credential envelopes,
authentication eligibility, scheduler metadata and import provenance. No account was imported from
the live grok2api deployment and no production graph, service or credential was changed.

Schema version 10 adds:

- `grok_accounts`, with provider-isolated identities, CPAR-encrypted credentials, auth status,
  priority, weight, concurrency, refresh/cooldown fields, revision and batch provenance;
- `grok_account_import_batches`, retaining applied/rolled-back state and value-free counts;
- `grok_account_links`, a metadata-only relationship whose foreign keys cascade on account
  rollback without merging health, quota or cooldown state.

## Import and rollback contract

- A source identity exists only in bounded, zeroizing memory. SQLite retains a provider-scoped
  SHA-256 digest, never the raw identity.
- Credential plaintext exists only in bounded, zeroizing memory and is immediately sealed by the
  existing `SecretStore` with provider-and-identity-bound AAD.
- Each batch is one SQLite transaction. A changed credential or scheduling conflict rolls back all
  rows and the batch audit row.
- An exact account imported under a new batch is counted as unchanged. Duplicate identities inside
  one batch fail before any write.
- Rollback removes only accounts created by the selected batch, cascades their links, retains the
  rolled-back batch row and is idempotent.
- Redacted listing omits identity digests and ciphertext. Runtime credential opening requires one
  exact random CPAR account ID and fails closed for a wrong key, wrong AAD or malformed envelope.

## Verification

- `cargo test --locked -p gateway-store`: PASS, including schema up/down and the updated one-version
  upgrade/backup/rollback drill.
- `cargo test --locked -p provider-grok`: PASS; P12-10B adds four tests covering all three provider
  namespaces, no plaintext persistence, redacted debug, idempotence, atomic conflict rollback,
  link cascade, invalid bounds and wrong-key failure.
- `cargo clippy --locked -p gateway-store -p provider-grok --all-targets -- -D warnings`: PASS.
- `cargo fmt --all -- --check`: PASS.
- `./scripts/check.sh docs`: PASS, including tracked secret scan and Git whitespace.

## Review

Review found and corrected one stale P11-07 assumption: after adding schema v10, a one-version
downgrade removes only the native Grok tables and correctly preserves the v9 management audit row.
Migration history remains a strict prefix, foreign keys and `quick_check` remain clean, and upgrade
restores the empty v10 tables without rewriting prior data.

P12-10B does not yet make accounts schedulable. P12-10C must compose eligible account metadata into
the existing `EndpointCredentialPools`, `RuntimeHealthRegistry` and `RuntimeQuotaRegistry` instead
of creating a second scheduler.
