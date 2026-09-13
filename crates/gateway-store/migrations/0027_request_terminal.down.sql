-- Refuse a lossy downgrade once new terminal observations exist.
CREATE TEMP TABLE request_terminal_rollback_guard (count INTEGER CHECK (count=0));
INSERT INTO request_terminal_rollback_guard SELECT COUNT(*) FROM gateway_event_log WHERE event_type='request_finished';
DROP TABLE request_terminal_rollback_guard;
DROP INDEX gateway_event_log_time;
DROP INDEX billing_ledger_request_idx;
DROP TRIGGER gateway_event_log_no_update;
DROP TRIGGER gateway_event_log_no_delete;
DROP INDEX gateway_event_log_request_id_ordinal;
DROP INDEX gateway_event_log_type_ordinal;
ALTER TABLE gateway_event_log RENAME TO gateway_event_log_previous;
CREATE TABLE gateway_event_log (
    event_ordinal INTEGER PRIMARY KEY,
    event_type TEXT NOT NULL CHECK (
        event_type IN ('request', 'attempt', 'usage', 'health')
    ),
    event_id TEXT NOT NULL CHECK (length(event_id) BETWEEN 1 AND 512),
    request_id TEXT CHECK (request_id IS NULL OR length(request_id) BETWEEN 1 AND 512),
    occurred_at_ms INTEGER,
    payload_json TEXT NOT NULL CHECK (length(payload_json) > 0),
    UNIQUE (event_type, event_id)
) STRICT;

CREATE INDEX gateway_event_log_request_id_ordinal
    ON gateway_event_log (request_id, event_ordinal)
    WHERE request_id IS NOT NULL;

CREATE INDEX gateway_event_log_type_ordinal
    ON gateway_event_log (event_type, event_ordinal);

CREATE TRIGGER gateway_event_log_no_update
BEFORE UPDATE ON gateway_event_log
BEGIN
    SELECT RAISE(ABORT, 'gateway event log is append-only');
END;

CREATE TRIGGER gateway_event_log_no_delete
BEFORE DELETE ON gateway_event_log
BEGIN
    SELECT RAISE(ABORT, 'gateway event log is append-only');
END;

INSERT INTO gateway_event_log SELECT * FROM gateway_event_log_previous;
DROP TABLE gateway_event_log_previous;
