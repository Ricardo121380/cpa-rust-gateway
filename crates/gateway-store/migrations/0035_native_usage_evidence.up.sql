CREATE TABLE native_usage_evidence (
    attempt_id TEXT PRIMARY KEY NOT NULL CHECK (length(attempt_id) BETWEEN 1 AND 512),
    request_id TEXT NOT NULL CHECK (length(request_id) BETWEEN 1 AND 512),
    client_key_id TEXT NOT NULL CHECK (length(client_key_id) BETWEEN 1 AND 512),
    source_event_id TEXT NOT NULL UNIQUE CHECK (length(source_event_id) BETWEEN 1 AND 512),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
    expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms > created_at_ms),
    payload_version INTEGER NOT NULL CHECK (payload_version = 1),
    key_version INTEGER NOT NULL CHECK (key_version > 0),
    ciphertext BLOB NOT NULL CHECK (length(ciphertext) BETWEEN 41 AND 524288)
) STRICT;
CREATE INDEX native_usage_evidence_expiry ON native_usage_evidence(expires_at_ms);
