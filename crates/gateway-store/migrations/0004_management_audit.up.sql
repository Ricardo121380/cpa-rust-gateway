CREATE TABLE management_audit_events (
    id INTEGER PRIMARY KEY,
    action TEXT NOT NULL CHECK (
        action IN ('config_created', 'config_published', 'config_rolled_back')
    ),
    actor TEXT NOT NULL CHECK (length(actor) BETWEEN 1 AND 128),
    occurred_at_ms INTEGER NOT NULL CHECK (occurred_at_ms >= 0),
    config_version_id TEXT NOT NULL,
    replaced_config_version_id TEXT,
    FOREIGN KEY (config_version_id) REFERENCES config_versions(id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    FOREIGN KEY (replaced_config_version_id) REFERENCES config_versions(id)
        ON UPDATE RESTRICT ON DELETE RESTRICT
) STRICT;

CREATE INDEX management_audit_events_config_version_id_id
    ON management_audit_events (config_version_id, id DESC);

CREATE TRIGGER management_audit_events_no_update
BEFORE UPDATE ON management_audit_events
BEGIN
    SELECT RAISE(ABORT, 'management audit events are append-only');
END;

CREATE TRIGGER management_audit_events_no_delete
BEFORE DELETE ON management_audit_events
BEGIN
    SELECT RAISE(ABORT, 'management audit events are append-only');
END;
