DROP TRIGGER gateway_event_log_no_update;
DROP TRIGGER gateway_event_log_no_delete;
DROP INDEX gateway_event_log_request_id_ordinal;
DROP INDEX gateway_event_log_type_ordinal;
ALTER TABLE gateway_event_log RENAME TO gateway_event_log_previous;
CREATE TABLE gateway_event_log (
    event_ordinal INTEGER PRIMARY KEY,
    event_type TEXT NOT NULL CHECK (
        event_type IN ('request', 'request_finished', 'attempt', 'usage', 'health')
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
CREATE INDEX gateway_event_log_time ON gateway_event_log(event_type,occurred_at_ms,event_ordinal);
CREATE INDEX billing_ledger_request_idx ON billing_ledger_entries(request_id,ledger_id);
