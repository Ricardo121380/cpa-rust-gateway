CREATE TABLE public_models (
    config_version_id TEXT NOT NULL CHECK (length(trim(config_version_id)) BETWEEN 1 AND 128),
    id TEXT NOT NULL CHECK (length(trim(id)) BETWEEN 1 AND 128),
    model_name TEXT NOT NULL CHECK (length(trim(model_name)) BETWEEN 1 AND 256),
    status TEXT NOT NULL CHECK (status IN ('active', 'disabled')),
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) BETWEEN 1 AND 256),
    capabilities_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(capabilities_json) AND json_type(capabilities_json) = 'object'),
    PRIMARY KEY (config_version_id, id),
    UNIQUE (config_version_id, model_name),
    FOREIGN KEY (config_version_id) REFERENCES config_versions(id) ON DELETE CASCADE
) STRICT;

CREATE TABLE model_aliases (
    config_version_id TEXT NOT NULL CHECK (length(trim(config_version_id)) BETWEEN 1 AND 128),
    alias TEXT NOT NULL CHECK (length(trim(alias)) BETWEEN 1 AND 256),
    public_model_id TEXT NOT NULL CHECK (length(trim(public_model_id)) BETWEEN 1 AND 128),
    PRIMARY KEY (config_version_id, alias),
    FOREIGN KEY (config_version_id, public_model_id)
        REFERENCES public_models(config_version_id, id) ON DELETE CASCADE
) STRICT;

CREATE TABLE model_routes (
    config_version_id TEXT NOT NULL CHECK (length(trim(config_version_id)) BETWEEN 1 AND 128),
    id TEXT NOT NULL CHECK (length(trim(id)) BETWEEN 1 AND 128),
    public_model_id TEXT NOT NULL CHECK (length(trim(public_model_id)) BETWEEN 1 AND 128),
    policy TEXT NOT NULL CHECK (policy IN ('round_robin', 'smooth_weighted_round_robin', 'priority_failover')),
    max_attempts INTEGER NOT NULL CHECK (max_attempts > 0),
    bootstrap_timeout_ms INTEGER NOT NULL CHECK (bootstrap_timeout_ms > 0),
    PRIMARY KEY (config_version_id, id),
    UNIQUE (config_version_id, public_model_id),
    FOREIGN KEY (config_version_id, public_model_id)
        REFERENCES public_models(config_version_id, id) ON DELETE CASCADE
) STRICT;

CREATE TABLE route_candidates (
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

CREATE INDEX route_candidates_by_route
ON route_candidates(config_version_id, route_id);

CREATE TABLE access_groups (
    config_version_id TEXT NOT NULL CHECK (length(trim(config_version_id)) BETWEEN 1 AND 128),
    id TEXT NOT NULL CHECK (length(trim(id)) BETWEEN 1 AND 128),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 256),
    status TEXT NOT NULL CHECK (status IN ('active', 'disabled')),
    limits_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(limits_json) AND json_type(limits_json) = 'object'),
    PRIMARY KEY (config_version_id, id),
    UNIQUE (config_version_id, name),
    FOREIGN KEY (config_version_id) REFERENCES config_versions(id) ON DELETE CASCADE
) STRICT;

CREATE TABLE access_group_routes (
    config_version_id TEXT NOT NULL CHECK (length(trim(config_version_id)) BETWEEN 1 AND 128),
    access_group_id TEXT NOT NULL CHECK (length(trim(access_group_id)) BETWEEN 1 AND 128),
    route_id TEXT NOT NULL CHECK (length(trim(route_id)) BETWEEN 1 AND 128),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    PRIMARY KEY (config_version_id, access_group_id, route_id),
    FOREIGN KEY (config_version_id, access_group_id)
        REFERENCES access_groups(config_version_id, id) ON DELETE CASCADE,
    FOREIGN KEY (config_version_id, route_id)
        REFERENCES model_routes(config_version_id, id) ON DELETE CASCADE
) STRICT;

CREATE INDEX access_group_routes_by_route
ON access_group_routes(config_version_id, route_id);

CREATE TABLE client_keys (
    config_version_id TEXT NOT NULL CHECK (length(trim(config_version_id)) BETWEEN 1 AND 128),
    id TEXT NOT NULL CHECK (length(trim(id)) BETWEEN 1 AND 128),
    prefix TEXT NOT NULL CHECK (length(trim(prefix)) BETWEEN 1 AND 128),
    secret_digest BLOB NOT NULL CHECK (length(secret_digest) = 32),
    access_group_id TEXT NOT NULL CHECK (length(trim(access_group_id)) BETWEEN 1 AND 128),
    status TEXT NOT NULL CHECK (status IN ('active', 'disabled', 'revoked')),
    expires_at_ms INTEGER CHECK (expires_at_ms IS NULL OR expires_at_ms >= 0),
    PRIMARY KEY (config_version_id, id),
    UNIQUE (config_version_id, prefix),
    UNIQUE (config_version_id, secret_digest),
    FOREIGN KEY (config_version_id, access_group_id)
        REFERENCES access_groups(config_version_id, id) ON DELETE CASCADE
) STRICT;

CREATE INDEX client_keys_by_access_group
ON client_keys(config_version_id, access_group_id);
