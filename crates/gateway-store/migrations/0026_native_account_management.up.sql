CREATE TABLE native_account_management_events (
    id INTEGER PRIMARY KEY,
    account_id TEXT NOT NULL,
    provider TEXT NOT NULL CHECK (provider IN ('web', 'console', 'build')),
    action TEXT NOT NULL CHECK (action IN ('enabled', 'disabled', 'credential_updated', 'removed')),
    revision INTEGER NOT NULL CHECK (revision >= 0),
    actor TEXT NOT NULL CHECK (length(actor) BETWEEN 1 AND 128),
    occurred_at_ms INTEGER NOT NULL CHECK (occurred_at_ms >= 0)
) STRICT;
CREATE INDEX native_account_management_events_account_idx ON native_account_management_events(account_id, id DESC);
CREATE TRIGGER native_account_management_events_no_update BEFORE UPDATE ON native_account_management_events BEGIN
    SELECT RAISE(ABORT, 'native account management events are append-only');
END;
CREATE TRIGGER native_account_management_events_no_delete BEFORE DELETE ON native_account_management_events BEGIN
    SELECT RAISE(ABORT, 'native account management events are append-only');
END;
