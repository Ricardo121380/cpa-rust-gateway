CREATE TABLE native_account_identity_observations (
    account_id TEXT PRIMARY KEY REFERENCES grok_accounts(id) ON DELETE CASCADE,
    credential_fingerprint BLOB NOT NULL CHECK (length(credential_fingerprint) = 32),
    ciphertext BLOB NOT NULL CHECK (length(ciphertext) > 0),
    key_version INTEGER NOT NULL CHECK (key_version > 0),
    observed_at_ms INTEGER NOT NULL CHECK (observed_at_ms >= 0)
) STRICT;
CREATE TRIGGER native_identity_insert AFTER INSERT ON native_account_identity_observations BEGIN
    UPDATE native_account_inventory_generation SET generation = generation + 1 WHERE singleton = 1;
END;
CREATE TRIGGER native_identity_update AFTER UPDATE ON native_account_identity_observations BEGIN
    UPDATE native_account_inventory_generation SET generation = generation + 1 WHERE singleton = 1;
END;
CREATE TRIGGER native_identity_delete AFTER DELETE ON native_account_identity_observations BEGIN
    UPDATE native_account_inventory_generation SET generation = generation + 1 WHERE singleton = 1;
END;
