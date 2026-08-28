CREATE TABLE config_versions (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) BETWEEN 1 AND 128),
    parent_id TEXT CHECK (parent_id IS NULL OR length(trim(parent_id)) BETWEEN 1 AND 128),
    status TEXT NOT NULL CHECK (status IN ('draft', 'active', 'archived')),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
    description TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 4096),
    FOREIGN KEY (parent_id) REFERENCES config_versions(id) ON DELETE RESTRICT,
    CHECK (parent_id IS NULL OR parent_id <> id)
) STRICT;

CREATE UNIQUE INDEX config_versions_one_active
ON config_versions(status)
WHERE status = 'active';

CREATE TABLE upstreams (
    config_version_id TEXT NOT NULL CHECK (length(trim(config_version_id)) BETWEEN 1 AND 128),
    id TEXT NOT NULL CHECK (length(trim(id)) BETWEEN 1 AND 128),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 256),
    kind TEXT NOT NULL CHECK (length(trim(kind)) BETWEEN 1 AND 128),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    tags_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(tags_json) AND json_type(tags_json) = 'array'),
    egress_policy_id TEXT CHECK (egress_policy_id IS NULL OR length(trim(egress_policy_id)) BETWEEN 1 AND 128),
    PRIMARY KEY (config_version_id, id),
    UNIQUE (config_version_id, name),
    FOREIGN KEY (config_version_id) REFERENCES config_versions(id) ON DELETE CASCADE
) STRICT;

CREATE TABLE upstream_endpoints (
    config_version_id TEXT NOT NULL CHECK (length(trim(config_version_id)) BETWEEN 1 AND 128),
    id TEXT NOT NULL CHECK (length(trim(id)) BETWEEN 1 AND 128),
    upstream_id TEXT NOT NULL CHECK (length(trim(upstream_id)) BETWEEN 1 AND 128),
    adapter_id TEXT NOT NULL CHECK (length(trim(adapter_id)) BETWEEN 1 AND 128),
    api_format TEXT NOT NULL CHECK (length(trim(api_format)) BETWEEN 1 AND 128),
    base_url TEXT NOT NULL CHECK (length(trim(base_url)) BETWEEN 1 AND 2048),
    inference_path TEXT NOT NULL CHECK (length(trim(inference_path)) BETWEEN 1 AND 1024),
    models_path TEXT CHECK (models_path IS NULL OR length(trim(models_path)) BETWEEN 1 AND 1024),
    transport TEXT NOT NULL CHECK (transport IN ('http', 'sse', 'websocket')),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    PRIMARY KEY (config_version_id, id),
    UNIQUE (config_version_id, id, upstream_id),
    FOREIGN KEY (config_version_id, upstream_id)
        REFERENCES upstreams(config_version_id, id) ON DELETE CASCADE
) STRICT;

CREATE INDEX upstream_endpoints_by_upstream
ON upstream_endpoints(config_version_id, upstream_id);

CREATE TABLE upstream_credentials (
    config_version_id TEXT NOT NULL CHECK (length(trim(config_version_id)) BETWEEN 1 AND 128),
    id TEXT NOT NULL CHECK (length(trim(id)) BETWEEN 1 AND 128),
    upstream_id TEXT NOT NULL CHECK (length(trim(upstream_id)) BETWEEN 1 AND 128),
    kind TEXT NOT NULL CHECK (length(trim(kind)) BETWEEN 1 AND 128),
    ciphertext BLOB NOT NULL CHECK (length(ciphertext) > 0),
    key_version INTEGER NOT NULL CHECK (key_version > 0),
    status TEXT NOT NULL CHECK (status IN ('active', 'cooling', 'unauthorized', 'disabled')),
    revision INTEGER NOT NULL CHECK (revision >= 0),
    PRIMARY KEY (config_version_id, id),
    UNIQUE (config_version_id, id, upstream_id),
    FOREIGN KEY (config_version_id, upstream_id)
        REFERENCES upstreams(config_version_id, id) ON DELETE CASCADE
) STRICT;

CREATE INDEX upstream_credentials_by_upstream
ON upstream_credentials(config_version_id, upstream_id);

CREATE TABLE endpoint_credential_bindings (
    config_version_id TEXT NOT NULL CHECK (length(trim(config_version_id)) BETWEEN 1 AND 128),
    endpoint_id TEXT NOT NULL CHECK (length(trim(endpoint_id)) BETWEEN 1 AND 128),
    credential_id TEXT NOT NULL CHECK (length(trim(credential_id)) BETWEEN 1 AND 128),
    upstream_id TEXT NOT NULL CHECK (length(trim(upstream_id)) BETWEEN 1 AND 128),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    priority INTEGER NOT NULL CHECK (priority >= 0),
    weight INTEGER NOT NULL CHECK (weight > 0),
    concurrency INTEGER NOT NULL CHECK (concurrency > 0),
    PRIMARY KEY (config_version_id, endpoint_id, credential_id),
    FOREIGN KEY (config_version_id, endpoint_id, upstream_id)
        REFERENCES upstream_endpoints(config_version_id, id, upstream_id) ON DELETE CASCADE,
    FOREIGN KEY (config_version_id, credential_id, upstream_id)
        REFERENCES upstream_credentials(config_version_id, id, upstream_id) ON DELETE CASCADE
) STRICT;

CREATE INDEX endpoint_credential_bindings_by_credential
ON endpoint_credential_bindings(config_version_id, credential_id);
