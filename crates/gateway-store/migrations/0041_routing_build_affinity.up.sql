ALTER TABLE model_routes ADD COLUMN selection_policy TEXT NOT NULL DEFAULT 'legacy'
    CHECK (selection_policy IN ('legacy', 'weighted_round_robin', 'fill_first'));
ALTER TABLE grok_build_cache_affinities ADD COLUMN binding_fingerprint TEXT
    CHECK (binding_fingerprint IS NULL OR length(binding_fingerprint) = 64);
ALTER TABLE grok_build_cache_affinities ADD COLUMN credential_revision INTEGER CHECK (credential_revision IS NULL OR credential_revision >= 0);
ALTER TABLE grok_build_affinity_breaks RENAME TO grok_build_affinity_breaks_v40;
CREATE TABLE grok_build_affinity_breaks (
    id INTEGER PRIMARY KEY,
    client_key_id TEXT NOT NULL CHECK (length(trim(client_key_id)) BETWEEN 1 AND 128),
    upstream_model TEXT NOT NULL CHECK (length(trim(upstream_model)) BETWEEN 1 AND 512),
    cache_identity TEXT NOT NULL CHECK (length(trim(cache_identity)) BETWEEN 1 AND 512),
    prior_credential_id TEXT NOT NULL CHECK (length(trim(prior_credential_id)) BETWEEN 1 AND 128),
    next_credential_id TEXT NOT NULL CHECK (length(trim(next_credential_id)) BETWEEN 1 AND 128),
    reason TEXT NOT NULL CHECK (reason IN ('expired', 'credential_unavailable', 'egress_changed', 'operator_rebind', 'policy_reselection')),
    estimated_cache_loss_tokens INTEGER CHECK (estimated_cache_loss_tokens IS NULL OR estimated_cache_loss_tokens >= 0),
    occurred_at_ms INTEGER NOT NULL
) STRICT;

INSERT INTO grok_build_affinity_breaks SELECT * FROM grok_build_affinity_breaks_v40;
DROP TABLE grok_build_affinity_breaks_v40;
