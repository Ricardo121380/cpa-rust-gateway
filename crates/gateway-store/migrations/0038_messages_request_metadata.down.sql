-- Refuse any rollback that could discard a Claude payload or expired receipt.
CREATE TEMP TABLE request_metadata_rollback_guard (remaining INTEGER CHECK (remaining = 0));
INSERT INTO request_metadata_rollback_guard SELECT count(*) FROM request_metadata_evidence WHERE payload_version = 2;
DROP TABLE request_metadata_rollback_guard;
CREATE TABLE request_metadata_evidence_v1 (
    request_id TEXT PRIMARY KEY NOT NULL CHECK (length(request_id) BETWEEN 1 AND 512),
    client_key_id TEXT NOT NULL CHECK (length(client_key_id) BETWEEN 1 AND 512),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
    expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms > created_at_ms),
    payload_version INTEGER NOT NULL CHECK (payload_version = 1),
    key_version INTEGER NOT NULL CHECK (key_version > 0),
    ciphertext BLOB CHECK (ciphertext IS NULL OR length(ciphertext) BETWEEN 41 AND 524288)
) STRICT;
INSERT INTO request_metadata_evidence_v1 SELECT * FROM request_metadata_evidence;
DROP TABLE request_metadata_evidence;
ALTER TABLE request_metadata_evidence_v1 RENAME TO request_metadata_evidence;
CREATE INDEX request_metadata_evidence_expiry ON request_metadata_evidence(expires_at_ms) WHERE ciphertext IS NOT NULL;
