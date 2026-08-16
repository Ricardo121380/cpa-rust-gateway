CREATE TABLE stored_response_compactions (
    client_key_id TEXT NOT NULL
        CHECK (length(client_key_id) BETWEEN 1 AND 128),
    compact_id TEXT NOT NULL
        CHECK (length(compact_id) BETWEEN 1 AND 512),
    created_at_ms INTEGER NOT NULL
        CHECK (created_at_ms >= 0),
    expires_at_ms INTEGER NOT NULL
        CHECK (expires_at_ms > created_at_ms),
    payload_version INTEGER NOT NULL
        CHECK (payload_version = 1),
    key_version INTEGER NOT NULL
        CHECK (key_version > 0),
    ciphertext BLOB NOT NULL
        CHECK (length(ciphertext) BETWEEN 41 AND 16777216),
    PRIMARY KEY (client_key_id, compact_id)
) STRICT;

CREATE INDEX stored_response_compactions_expiry_idx
    ON stored_response_compactions (expires_at_ms, client_key_id, compact_id);
