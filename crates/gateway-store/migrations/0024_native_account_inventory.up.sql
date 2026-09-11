CREATE TABLE native_account_inventory_generation (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    generation INTEGER NOT NULL CHECK (generation >= 0)
) STRICT;
INSERT INTO native_account_inventory_generation VALUES (1, 0);
CREATE TRIGGER native_account_inventory_insert AFTER INSERT ON grok_accounts BEGIN
    UPDATE native_account_inventory_generation SET generation = generation + 1 WHERE singleton = 1;
END;
CREATE TRIGGER native_account_inventory_delete AFTER DELETE ON grok_accounts BEGIN
    UPDATE native_account_inventory_generation SET generation = generation + 1 WHERE singleton = 1;
END;
CREATE TRIGGER native_account_inventory_update AFTER UPDATE OF id, provider, auth_status, enabled, priority, weight, max_concurrency, refresh_due_at_ms, quota_sync_due_at_ms, cooldown_until_ms, revision, import_batch_id ON grok_accounts BEGIN
    UPDATE native_account_inventory_generation SET generation = generation + 1 WHERE singleton = 1;
END;

CREATE TABLE native_account_authorization_events (
    id INTEGER PRIMARY KEY,
    account_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 0),
    action TEXT NOT NULL CHECK (action = 'device_reauthorized'),
    occurred_at_ms INTEGER NOT NULL CHECK (occurred_at_ms >= 0)
) STRICT;
