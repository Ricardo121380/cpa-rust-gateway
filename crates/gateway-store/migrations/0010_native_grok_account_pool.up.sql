CREATE TABLE grok_account_import_batches (
    id TEXT PRIMARY KEY CHECK (
        length(trim(id)) BETWEEN 1 AND 128
        AND length(CAST(id AS BLOB)) <= 128
    ),
    status TEXT NOT NULL CHECK (status IN ('applied', 'rolled_back')),
    created_count INTEGER NOT NULL CHECK (created_count >= 0),
    unchanged_count INTEGER NOT NULL CHECK (unchanged_count >= 0),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
    rolled_back_at_ms INTEGER CHECK (
        rolled_back_at_ms IS NULL OR rolled_back_at_ms >= created_at_ms
    )
) STRICT;

CREATE TABLE grok_accounts (
    id TEXT PRIMARY KEY CHECK (
        length(trim(id)) BETWEEN 1 AND 128
        AND length(CAST(id AS BLOB)) <= 128
    ),
    provider TEXT NOT NULL CHECK (provider IN ('build', 'web', 'console')),
    identity_digest BLOB NOT NULL CHECK (length(identity_digest) = 32),
    credential_ciphertext BLOB NOT NULL CHECK (length(credential_ciphertext) > 0),
    credential_key_version INTEGER NOT NULL CHECK (credential_key_version > 0),
    auth_status TEXT NOT NULL CHECK (
        auth_status IN ('active', 'reauth_required', 'disabled')
    ),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    priority INTEGER NOT NULL CHECK (priority BETWEEN -1000 AND 1000),
    weight INTEGER NOT NULL CHECK (weight BETWEEN 1 AND 10000),
    max_concurrency INTEGER NOT NULL CHECK (max_concurrency BETWEEN 1 AND 10000),
    refresh_due_at_ms INTEGER CHECK (refresh_due_at_ms IS NULL OR refresh_due_at_ms >= 0),
    last_refresh_at_ms INTEGER CHECK (last_refresh_at_ms IS NULL OR last_refresh_at_ms >= 0),
    refresh_failure_count INTEGER NOT NULL DEFAULT 0 CHECK (refresh_failure_count >= 0),
    cooldown_until_ms INTEGER CHECK (cooldown_until_ms IS NULL OR cooldown_until_ms >= 0),
    revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0),
    import_batch_id TEXT NOT NULL REFERENCES grok_account_import_batches(id),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= created_at_ms),
    UNIQUE (provider, identity_digest)
) STRICT;

CREATE INDEX grok_accounts_provider_eligibility_idx
    ON grok_accounts (
        provider,
        enabled,
        auth_status,
        priority DESC,
        cooldown_until_ms,
        refresh_due_at_ms
    );

CREATE INDEX grok_accounts_import_batch_idx
    ON grok_accounts (import_batch_id, id);

CREATE TABLE grok_account_links (
    source_account_id TEXT NOT NULL REFERENCES grok_accounts(id) ON DELETE CASCADE,
    target_account_id TEXT NOT NULL REFERENCES grok_accounts(id) ON DELETE CASCADE,
    relation TEXT NOT NULL CHECK (
        length(trim(relation)) BETWEEN 1 AND 64
        AND length(CAST(relation AS BLOB)) <= 64
    ),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
    CHECK (source_account_id <> target_account_id),
    PRIMARY KEY (source_account_id, target_account_id, relation)
) STRICT;
