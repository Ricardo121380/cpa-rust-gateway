-- Never erase explicit strategies, serving fingerprints or UNKNOWN cache loss on downgrade.
CREATE TEMP TABLE issue70_downgrade_guard (ok INTEGER CHECK (ok = 1));
INSERT INTO issue70_downgrade_guard SELECT CASE WHEN
    EXISTS (SELECT 1 FROM model_routes WHERE selection_policy != 'legacy') OR
    EXISTS (SELECT 1 FROM grok_build_cache_affinities WHERE binding_fingerprint IS NOT NULL) OR
    EXISTS (SELECT 1 FROM grok_build_affinity_breaks WHERE estimated_cache_loss_tokens IS NULL OR reason='policy_reselection')
    THEN 0 ELSE 1 END;
DROP TABLE issue70_downgrade_guard;
ALTER TABLE model_routes DROP COLUMN selection_policy;
ALTER TABLE grok_build_cache_affinities DROP COLUMN binding_fingerprint;
ALTER TABLE grok_build_cache_affinities DROP COLUMN credential_revision;
ALTER TABLE grok_build_affinity_breaks RENAME TO grok_build_affinity_breaks_v41;
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

INSERT INTO grok_build_affinity_breaks SELECT * FROM grok_build_affinity_breaks_v41;
DROP TABLE grok_build_affinity_breaks_v41;
