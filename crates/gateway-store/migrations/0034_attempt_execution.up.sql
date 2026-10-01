CREATE TABLE gateway_attempt_execution (
    event_type TEXT NOT NULL DEFAULT 'attempt' CHECK (event_type = 'attempt'),
    attempt_id TEXT PRIMARY KEY CHECK (length(attempt_id) BETWEEN 1 AND 512),
    request_id TEXT NOT NULL CHECK (length(request_id) BETWEEN 1 AND 512),
    attempt_payload_sha256 TEXT NOT NULL CHECK (length(attempt_payload_sha256) = 64),
    evidence_json TEXT NOT NULL CHECK (length(evidence_json) <= 8192 AND json_valid(evidence_json)),
    FOREIGN KEY (event_type, attempt_id) REFERENCES gateway_event_log(event_type, event_id)
) STRICT;
CREATE TRIGGER gateway_attempt_execution_no_update BEFORE UPDATE ON gateway_attempt_execution
BEGIN SELECT RAISE(ABORT, 'attempt execution evidence is append-only'); END;
CREATE TRIGGER gateway_attempt_execution_no_delete BEFORE DELETE ON gateway_attempt_execution
BEGIN SELECT RAISE(ABORT, 'attempt execution evidence is append-only'); END;
