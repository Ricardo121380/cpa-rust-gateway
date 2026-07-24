CREATE TABLE grok_build_credential_runtime (
    config_version_id TEXT NOT NULL CHECK (
        length(trim(config_version_id)) BETWEEN 1 AND 128
        AND length(CAST(config_version_id AS BLOB)) <= 128
    ),
    credential_id TEXT NOT NULL CHECK (
        length(trim(credential_id)) BETWEEN 1 AND 128
        AND length(CAST(credential_id AS BLOB)) <= 128
    ),
    revision INTEGER NOT NULL CHECK (revision >= 0),
    ciphertext BLOB NOT NULL CHECK (length(ciphertext) > 0),
    key_version INTEGER NOT NULL CHECK (key_version > 0),
    updated_at_ms INTEGER NOT NULL,
    PRIMARY KEY (config_version_id, credential_id)
) STRICT;

CREATE INDEX grok_build_credential_runtime_credential_idx
    ON grok_build_credential_runtime (credential_id, config_version_id);
