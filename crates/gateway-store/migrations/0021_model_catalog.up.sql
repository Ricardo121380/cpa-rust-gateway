CREATE TABLE model_catalog_targets (
    config_version_id TEXT NOT NULL CHECK (length(config_version_id) > 0),
    endpoint_id TEXT NOT NULL CHECK (length(endpoint_id) > 0),
    credential_id TEXT NOT NULL CHECK (length(credential_id) > 0),
    snapshot_version INTEGER NOT NULL CHECK (snapshot_version > 0),
    observed_at_ms INTEGER NOT NULL CHECK (observed_at_ms >= 0),
    stale_at_ms INTEGER NOT NULL CHECK (stale_at_ms > observed_at_ms),
    refresh_due_at_ms INTEGER NOT NULL CHECK (refresh_due_at_ms >= stale_at_ms),
    expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms > refresh_due_at_ms),
    PRIMARY KEY (config_version_id, endpoint_id, credential_id)
) STRICT;

CREATE TABLE model_catalog_failures (
    config_version_id TEXT NOT NULL CHECK (length(config_version_id) > 0),
    endpoint_id TEXT NOT NULL CHECK (length(endpoint_id) > 0),
    credential_id TEXT NOT NULL CHECK (length(credential_id) > 0),
    failed_at_ms INTEGER NOT NULL CHECK (failed_at_ms >= 0),
    failure_class TEXT NOT NULL CHECK (
        failure_class IN ('authentication', 'authorization', 'rate_limit', 'transport', 'upstream', 'internal')
    ),
    PRIMARY KEY (config_version_id, endpoint_id, credential_id)
) STRICT;

CREATE TABLE model_catalog_models (
    config_version_id TEXT NOT NULL,
    endpoint_id TEXT NOT NULL,
    credential_id TEXT NOT NULL,
    model TEXT NOT NULL CHECK (length(model) > 0),
    present_in_last_success INTEGER NOT NULL CHECK (present_in_last_success IN (0, 1)),
    consecutive_successful_misses INTEGER NOT NULL DEFAULT 0 CHECK (consecutive_successful_misses >= 0),
    first_missing_at_ms INTEGER CHECK (first_missing_at_ms >= 0),
    removal_eligible_at_ms INTEGER CHECK (removal_eligible_at_ms >= 0),
    CHECK (
        (present_in_last_success = 1 AND consecutive_successful_misses = 0
            AND first_missing_at_ms IS NULL AND removal_eligible_at_ms IS NULL)
        OR
        (present_in_last_success = 0 AND consecutive_successful_misses > 0
            AND first_missing_at_ms IS NOT NULL AND removal_eligible_at_ms IS NOT NULL
            AND removal_eligible_at_ms > first_missing_at_ms)
    ),
    PRIMARY KEY (config_version_id, endpoint_id, credential_id, model),
    FOREIGN KEY (config_version_id, endpoint_id, credential_id)
        REFERENCES model_catalog_targets(config_version_id, endpoint_id, credential_id)
        ON DELETE CASCADE
) STRICT;

CREATE INDEX model_catalog_targets_by_config
ON model_catalog_targets(config_version_id, endpoint_id, credential_id);
