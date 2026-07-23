CREATE TABLE grok_build_billing_profiles (
    credential_id TEXT PRIMARY KEY CHECK (
        length(trim(credential_id)) BETWEEN 1 AND 128
        AND length(CAST(credential_id AS BLOB)) <= 128
    ),
    plan_kind TEXT NOT NULL CHECK (plan_kind IN ('free', 'pay_as_you_go', 'subscription')),
    observed_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE grok_build_model_catalog (
    credential_id TEXT NOT NULL REFERENCES grok_build_billing_profiles (credential_id) ON DELETE CASCADE,
    upstream_model TEXT NOT NULL CHECK (
        length(trim(upstream_model)) BETWEEN 1 AND 512
        AND length(CAST(upstream_model AS BLOB)) <= 512
    ),
    source TEXT NOT NULL CHECK (source IN ('account_capability', 'build_response')),
    observed_at_ms INTEGER NOT NULL,
    PRIMARY KEY (credential_id, upstream_model)
) STRICT;

CREATE TABLE grok_build_quota_windows (
    credential_id TEXT NOT NULL REFERENCES grok_build_billing_profiles (credential_id) ON DELETE CASCADE,
    window_kind TEXT NOT NULL CHECK (window_kind IN ('free', 'pay_as_you_go', 'subscription_monthly', 'web_weekly')),
    remaining INTEGER NOT NULL CHECK (remaining >= 0),
    total INTEGER NOT NULL CHECK (total > 0 AND remaining <= total),
    window_seconds INTEGER NOT NULL CHECK (window_seconds > 0),
    reset_at_ms INTEGER NOT NULL,
    observed_at_ms INTEGER NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('billing', 'response_headers', 'web_rest', 'web_grpc_web', 'local_estimate')),
    confidence TEXT NOT NULL CHECK (confidence IN ('authoritative', 'observed', 'estimated')),
    raw_window_type TEXT NOT NULL CHECK (
        length(trim(raw_window_type)) BETWEEN 1 AND 128
        AND length(CAST(raw_window_type AS BLOB)) <= 128
    ),
    PRIMARY KEY (credential_id, window_kind)
) STRICT;

CREATE TABLE grok_build_cache_affinities (
    client_key_id TEXT NOT NULL CHECK (length(trim(client_key_id)) BETWEEN 1 AND 128),
    provider_id TEXT NOT NULL CHECK (provider_id = 'grok.build'),
    upstream_model TEXT NOT NULL CHECK (length(trim(upstream_model)) BETWEEN 1 AND 512),
    cache_identity TEXT NOT NULL CHECK (length(trim(cache_identity)) BETWEEN 1 AND 512),
    credential_id TEXT NOT NULL CHECK (length(trim(credential_id)) BETWEEN 1 AND 128),
    egress_policy_id TEXT CHECK (egress_policy_id IS NULL OR length(trim(egress_policy_id)) BETWEEN 1 AND 128),
    expires_at_ms INTEGER NOT NULL,
    reason TEXT NOT NULL CHECK (reason IN ('prompt_cache', 'server_requested', 'response_continuation')),
    updated_at_ms INTEGER NOT NULL,
    PRIMARY KEY (client_key_id, provider_id, upstream_model, cache_identity)
) STRICT;

CREATE TABLE grok_build_affinity_breaks (
    id INTEGER PRIMARY KEY,
    client_key_id TEXT NOT NULL CHECK (length(trim(client_key_id)) BETWEEN 1 AND 128),
    upstream_model TEXT NOT NULL CHECK (length(trim(upstream_model)) BETWEEN 1 AND 512),
    cache_identity TEXT NOT NULL CHECK (length(trim(cache_identity)) BETWEEN 1 AND 512),
    prior_credential_id TEXT NOT NULL CHECK (length(trim(prior_credential_id)) BETWEEN 1 AND 128),
    next_credential_id TEXT NOT NULL CHECK (length(trim(next_credential_id)) BETWEEN 1 AND 128),
    reason TEXT NOT NULL CHECK (reason IN ('expired', 'credential_unavailable', 'egress_changed', 'operator_rebind')),
    estimated_cache_loss_tokens INTEGER NOT NULL CHECK (estimated_cache_loss_tokens >= 0),
    occurred_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE grok_build_response_ownership (
    client_key_id TEXT NOT NULL CHECK (length(trim(client_key_id)) BETWEEN 1 AND 128),
    downstream_response_id TEXT NOT NULL CHECK (length(trim(downstream_response_id)) BETWEEN 1 AND 512),
    provider_id TEXT NOT NULL CHECK (provider_id = 'grok.build'),
    credential_id TEXT NOT NULL CHECK (length(trim(credential_id)) BETWEEN 1 AND 128),
    upstream_response_id TEXT NOT NULL CHECK (length(trim(upstream_response_id)) BETWEEN 1 AND 512),
    expires_at_ms INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY (client_key_id, downstream_response_id)
) STRICT;

CREATE TABLE grok_build_reasoning_replay (
    client_key_id TEXT NOT NULL CHECK (length(trim(client_key_id)) BETWEEN 1 AND 128),
    provider_id TEXT NOT NULL CHECK (provider_id = 'grok.build'),
    upstream_model TEXT NOT NULL CHECK (length(trim(upstream_model)) BETWEEN 1 AND 512),
    session_id TEXT NOT NULL CHECK (length(trim(session_id)) BETWEEN 1 AND 512),
    signature TEXT NOT NULL CHECK (signature = 'grok-build-responses-v1'),
    ciphertext BLOB NOT NULL CHECK (length(ciphertext) > 0),
    key_version INTEGER NOT NULL CHECK (key_version > 0),
    expires_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    PRIMARY KEY (client_key_id, provider_id, upstream_model, session_id)
) STRICT;

CREATE INDEX grok_build_quota_windows_reset_idx
    ON grok_build_quota_windows (credential_id, reset_at_ms);
CREATE INDEX grok_build_cache_affinities_expiry_idx
    ON grok_build_cache_affinities (expires_at_ms);
CREATE INDEX grok_build_response_ownership_expiry_idx
    ON grok_build_response_ownership (expires_at_ms);
