ALTER TABLE grok_account_import_batches ADD COLUMN updated_count INTEGER NOT NULL DEFAULT 0 CHECK (updated_count >= 0);
CREATE TABLE native_account_import_receipts (
    batch_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    provider TEXT NOT NULL CHECK (provider IN ('build', 'console', 'web')),
    credential_revision INTEGER NOT NULL CHECK (credential_revision >= 0),
    outcome TEXT NOT NULL CHECK (outcome IN ('created', 'unchanged', 'updated')),
    saved_at_ms INTEGER NOT NULL CHECK (saved_at_ms >= 0),
    PRIMARY KEY (batch_id, account_id),
    FOREIGN KEY (batch_id) REFERENCES grok_account_import_batches(id) ON DELETE CASCADE
) STRICT;
-- Account deletion retains import evidence; account_id deliberately has no foreign key.
