CREATE TABLE management_resource_audit_events (
    id INTEGER PRIMARY KEY,
    action TEXT NOT NULL CHECK (length(trim(action)) BETWEEN 1 AND 64),
    actor TEXT NOT NULL CHECK (length(trim(actor)) BETWEEN 1 AND 128),
    occurred_at_ms INTEGER NOT NULL CHECK (occurred_at_ms >= 0),
    config_version_id TEXT NOT NULL CHECK (length(trim(config_version_id)) BETWEEN 1 AND 128),
    resource_kind TEXT NOT NULL CHECK (length(trim(resource_kind)) BETWEEN 1 AND 64),
    resource_id TEXT NOT NULL CHECK (length(trim(resource_id)) BETWEEN 1 AND 128),
    FOREIGN KEY (config_version_id) REFERENCES config_versions(id)
        ON UPDATE RESTRICT ON DELETE RESTRICT
) STRICT;

CREATE INDEX management_resource_audit_events_config_version_id_id
    ON management_resource_audit_events (config_version_id, id DESC);

CREATE TRIGGER management_resource_audit_events_no_update
BEFORE UPDATE ON management_resource_audit_events
BEGIN
    SELECT RAISE(ABORT, 'management resource audit events are append-only');
END;

CREATE TRIGGER management_resource_audit_events_no_delete
BEFORE DELETE ON management_resource_audit_events
BEGIN
    SELECT RAISE(ABORT, 'management resource audit events are append-only');
END;
