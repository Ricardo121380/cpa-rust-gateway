CREATE TABLE billing_price_catalog_versions (
    catalog_version_id TEXT PRIMARY KEY CHECK (
        length(trim(catalog_version_id)) BETWEEN 1 AND 128
        AND length(CAST(catalog_version_id AS BLOB)) <= 128
    ),
    effective_at_ms INTEGER NOT NULL CHECK (effective_at_ms >= 0),
    source TEXT NOT NULL CHECK (source IN ('operator', 'imported', 'test')),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0)
) STRICT;

CREATE TABLE billing_price_catalog_entries (
    catalog_version_id TEXT NOT NULL REFERENCES billing_price_catalog_versions (catalog_version_id)
        ON DELETE CASCADE,
    provider_id TEXT NOT NULL CHECK (
        length(trim(provider_id)) BETWEEN 1 AND 128
        AND length(CAST(provider_id AS BLOB)) <= 128
    ),
    channel_id TEXT NOT NULL CHECK (
        length(trim(channel_id)) BETWEEN 1 AND 128
        AND length(CAST(channel_id AS BLOB)) <= 128
    ),
    model TEXT NOT NULL CHECK (
        length(trim(model)) BETWEEN 1 AND 512
        AND length(CAST(model AS BLOB)) <= 512
    ),
    input_microunits_per_million INTEGER NOT NULL CHECK (input_microunits_per_million >= 0),
    output_microunits_per_million INTEGER NOT NULL CHECK (output_microunits_per_million >= 0),
    reasoning_microunits_per_million INTEGER NOT NULL CHECK (reasoning_microunits_per_million >= 0),
    cache_read_microunits_per_million INTEGER NOT NULL CHECK (cache_read_microunits_per_million >= 0),
    cache_creation_microunits_per_million INTEGER NOT NULL CHECK (cache_creation_microunits_per_million >= 0),
    cached_microunits_per_million INTEGER NOT NULL CHECK (cached_microunits_per_million >= 0),
    PRIMARY KEY (catalog_version_id, provider_id, channel_id, model)
) STRICT;

CREATE INDEX billing_price_catalog_effective_idx
    ON billing_price_catalog_versions (effective_at_ms DESC, catalog_version_id);

CREATE TABLE billing_ledger_entries (
    ledger_id INTEGER PRIMARY KEY,
    source_event_id TEXT NOT NULL UNIQUE CHECK (
        length(trim(source_event_id)) BETWEEN 1 AND 512
        AND length(CAST(source_event_id AS BLOB)) <= 512
    ),
    source_fingerprint TEXT NOT NULL CHECK (
        length(source_fingerprint) = 64
        AND source_fingerprint GLOB '[0-9a-f]*'
    ),
    request_id TEXT NOT NULL CHECK (
        length(trim(request_id)) BETWEEN 1 AND 512
        AND length(CAST(request_id AS BLOB)) <= 512
    ),
    response_id TEXT NOT NULL CHECK (
        length(trim(response_id)) BETWEEN 1 AND 512
        AND length(CAST(response_id AS BLOB)) <= 512
    ),
    provider_id TEXT NOT NULL CHECK (
        length(trim(provider_id)) BETWEEN 1 AND 128
        AND length(CAST(provider_id AS BLOB)) <= 128
    ),
    channel_id TEXT NOT NULL CHECK (
        length(trim(channel_id)) BETWEEN 1 AND 128
        AND length(CAST(channel_id AS BLOB)) <= 128
    ),
    account_id TEXT NOT NULL CHECK (
        length(trim(account_id)) BETWEEN 1 AND 128
        AND length(CAST(account_id AS BLOB)) <= 128
    ),
    model TEXT NOT NULL CHECK (
        length(trim(model)) BETWEEN 1 AND 512
        AND length(CAST(model AS BLOB)) <= 512
    ),
    occurred_at_ms INTEGER NOT NULL CHECK (occurred_at_ms >= 0),
    catalog_version_id TEXT REFERENCES billing_price_catalog_versions (catalog_version_id),
    input_tokens INTEGER CHECK (input_tokens IS NULL OR input_tokens >= 0),
    output_tokens INTEGER CHECK (output_tokens IS NULL OR output_tokens >= 0),
    reasoning_tokens INTEGER CHECK (reasoning_tokens IS NULL OR reasoning_tokens >= 0),
    cache_read_tokens INTEGER CHECK (cache_read_tokens IS NULL OR cache_read_tokens >= 0),
    cache_creation_tokens INTEGER CHECK (cache_creation_tokens IS NULL OR cache_creation_tokens >= 0),
    cached_tokens INTEGER CHECK (cached_tokens IS NULL OR cached_tokens >= 0),
    cost_microunits INTEGER CHECK (cost_microunits IS NULL OR cost_microunits >= 0),
    cost_confidence TEXT NOT NULL CHECK (cost_confidence IN ('exact', 'partial', 'unknown', 'unpriced')),
    billing_status TEXT NOT NULL CHECK (billing_status IN ('priced', 'partial', 'unknown', 'unpriced')),
    retention_expires_at_ms INTEGER NOT NULL CHECK (retention_expires_at_ms >= occurred_at_ms),
    recorded_at_ms INTEGER NOT NULL CHECK (recorded_at_ms >= 0)
) STRICT;

CREATE INDEX billing_ledger_occurred_idx
    ON billing_ledger_entries (occurred_at_ms, ledger_id);
CREATE INDEX billing_ledger_retention_idx
    ON billing_ledger_entries (retention_expires_at_ms, ledger_id);
