CREATE TABLE credential_refresh_state (
    config_version_id TEXT NOT NULL,
    credential_id TEXT NOT NULL,
    credential_revision INTEGER NOT NULL CHECK (credential_revision >= 0),
    claim_id TEXT,
    lease_until_ms INTEGER NOT NULL DEFAULT 0 CHECK (lease_until_ms >= 0),
    retry_after_ms INTEGER NOT NULL DEFAULT 0 CHECK (retry_after_ms >= 0),
    failure_count INTEGER NOT NULL DEFAULT 0 CHECK (failure_count >= 0),
    last_error TEXT CHECK (last_error IN ('network', 'invalid_response', 'reauth_required')),
    observed_at_ms INTEGER NOT NULL CHECK (observed_at_ms >= 0),
    PRIMARY KEY (config_version_id, credential_id),
    FOREIGN KEY (config_version_id, credential_id) REFERENCES upstream_credentials(config_version_id, id)
        ON DELETE CASCADE ON UPDATE RESTRICT
) STRICT;
ALTER TABLE grok_accounts ADD COLUMN continuation_floor_revision INTEGER CHECK (continuation_floor_revision > 0);
ALTER TABLE grok_accounts ADD COLUMN continuation_current_revision INTEGER CHECK ((continuation_floor_revision IS NULL AND continuation_current_revision IS NULL) OR (continuation_floor_revision IS NOT NULL AND continuation_current_revision IS NOT NULL AND continuation_current_revision >= continuation_floor_revision));
