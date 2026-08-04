CREATE TABLE route_candidates_without_canonical_bridge (
    config_version_id TEXT NOT NULL CHECK (length(trim(config_version_id)) BETWEEN 1 AND 128),
    id TEXT NOT NULL CHECK (length(trim(id)) BETWEEN 1 AND 128),
    route_id TEXT NOT NULL CHECK (length(trim(route_id)) BETWEEN 1 AND 128),
    endpoint_id TEXT NOT NULL CHECK (length(trim(endpoint_id)) BETWEEN 1 AND 128),
    upstream_model TEXT NOT NULL CHECK (length(trim(upstream_model)) BETWEEN 1 AND 256),
    credential_scope TEXT NOT NULL CHECK (credential_scope = 'endpoint_bindings'),
    transform_mode TEXT NOT NULL CHECK (transform_mode IN ('passthrough', 'canonical', 'lossless_bridge')),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    priority INTEGER NOT NULL CHECK (priority >= 0),
    weight INTEGER NOT NULL CHECK (weight > 0),
    capability_override_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(capability_override_json) AND json_type(capability_override_json) = 'object'),
    PRIMARY KEY (config_version_id, id),
    UNIQUE (config_version_id, route_id, endpoint_id, upstream_model, credential_scope),
    FOREIGN KEY (config_version_id, route_id)
        REFERENCES model_routes(config_version_id, id) ON DELETE CASCADE,
    FOREIGN KEY (config_version_id, endpoint_id)
        REFERENCES upstream_endpoints(config_version_id, id) ON DELETE CASCADE
) STRICT;

INSERT INTO route_candidates_without_canonical_bridge (
    config_version_id, id, route_id, endpoint_id, upstream_model, credential_scope,
    transform_mode, enabled, priority, weight, capability_override_json
)
SELECT
    config_version_id, id, route_id, endpoint_id, upstream_model, credential_scope,
    transform_mode, enabled, priority, weight, capability_override_json
FROM route_candidates;

DROP TABLE route_candidates;
ALTER TABLE route_candidates_without_canonical_bridge RENAME TO route_candidates;

CREATE INDEX route_candidates_by_route
ON route_candidates(config_version_id, route_id);
