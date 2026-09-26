CREATE TABLE gateway_request_recording (
    request_id TEXT PRIMARY KEY,
    started_at_ms INTEGER NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('active', 'finished', 'interrupted'))
) STRICT;
CREATE INDEX gateway_request_recording_state ON gateway_request_recording(state);
CREATE TABLE gateway_event_quarantine (
    fingerprint TEXT PRIMARY KEY CHECK(length(fingerprint)=64),
    event_kind TEXT NOT NULL,
    reason TEXT NOT NULL CHECK(reason='invalid_or_conflicting_event'),
    payload_json TEXT,
    quarantined_at_ms INTEGER NOT NULL
) STRICT;
