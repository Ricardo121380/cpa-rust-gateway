CREATE TABLE compatible_egress_proxy_pools (
    config_version_id TEXT NOT NULL CHECK (length(trim(config_version_id)) BETWEEN 1 AND 128),
    id TEXT NOT NULL CHECK (length(trim(id)) BETWEEN 1 AND 128),
    upstream_id TEXT NOT NULL CHECK (length(trim(upstream_id)) BETWEEN 1 AND 128),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 256),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    PRIMARY KEY (config_version_id, id),
    UNIQUE (config_version_id, id, upstream_id),
    UNIQUE (config_version_id, upstream_id, name),
    FOREIGN KEY (config_version_id, upstream_id)
        REFERENCES upstreams(config_version_id, id) ON DELETE CASCADE
) STRICT;

CREATE INDEX compatible_egress_proxy_pools_by_upstream
ON compatible_egress_proxy_pools(config_version_id, upstream_id, id);

CREATE TABLE compatible_egress_proxy_nodes (
    config_version_id TEXT NOT NULL CHECK (length(trim(config_version_id)) BETWEEN 1 AND 128),
    id TEXT NOT NULL CHECK (length(trim(id)) BETWEEN 1 AND 128),
    upstream_id TEXT NOT NULL CHECK (length(trim(upstream_id)) BETWEEN 1 AND 128),
    pool_id TEXT CHECK (pool_id IS NULL OR length(trim(pool_id)) BETWEEN 1 AND 128),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 256),
    ciphertext BLOB NOT NULL CHECK (length(ciphertext) > 0),
    key_version INTEGER NOT NULL CHECK (key_version > 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    weight INTEGER NOT NULL CHECK (weight BETWEEN 1 AND 1024),
    maximum_concurrency INTEGER NOT NULL CHECK (maximum_concurrency BETWEEN 1 AND 100000),
    CHECK (pool_id IS NOT NULL OR weight = 1),
    PRIMARY KEY (config_version_id, id),
    UNIQUE (config_version_id, id, upstream_id),
    UNIQUE (config_version_id, upstream_id, name),
    FOREIGN KEY (config_version_id, upstream_id)
        REFERENCES upstreams(config_version_id, id) ON DELETE CASCADE,
    FOREIGN KEY (config_version_id, pool_id, upstream_id)
        REFERENCES compatible_egress_proxy_pools(config_version_id, id, upstream_id)
        ON DELETE CASCADE
) STRICT;

CREATE INDEX compatible_egress_proxy_nodes_by_pool
ON compatible_egress_proxy_nodes(config_version_id, upstream_id, pool_id, id);

CREATE TABLE compatible_egress_binding_profiles (
    config_version_id TEXT NOT NULL CHECK (length(trim(config_version_id)) BETWEEN 1 AND 128),
    endpoint_id TEXT NOT NULL CHECK (length(trim(endpoint_id)) BETWEEN 1 AND 128),
    credential_id TEXT NOT NULL CHECK (length(trim(credential_id)) BETWEEN 1 AND 128),
    target_kind TEXT NOT NULL CHECK (target_kind IN ('direct', 'fixed_proxy', 'proxy_pool')),
    target_id TEXT CHECK (target_id IS NULL OR length(trim(target_id)) BETWEEN 1 AND 128),
    failure_scope TEXT NOT NULL CHECK (failure_scope IN ('endpoint', 'credential', 'egress_node')),
    stickiness TEXT NOT NULL CHECK (stickiness IN ('none', 'credential', 'credential_and_egress')),
    pre_submit_max_attempts INTEGER NOT NULL CHECK (pre_submit_max_attempts BETWEEN 1 AND 3),
    CHECK (
        (target_kind = 'direct' AND target_id IS NULL)
        OR (target_kind IN ('fixed_proxy', 'proxy_pool') AND target_id IS NOT NULL)
    ),
    CHECK (target_kind != 'direct' OR failure_scope != 'egress_node'),
    CHECK (target_kind != 'direct' OR stickiness != 'credential_and_egress'),
    PRIMARY KEY (config_version_id, endpoint_id, credential_id),
    FOREIGN KEY (config_version_id, endpoint_id, credential_id)
        REFERENCES endpoint_credential_bindings(config_version_id, endpoint_id, credential_id)
        ON DELETE CASCADE
) STRICT;

CREATE TRIGGER compatible_egress_binding_fixed_target_insert
BEFORE INSERT ON compatible_egress_binding_profiles
WHEN NEW.target_kind = 'fixed_proxy'
 AND NOT EXISTS (
    SELECT 1
    FROM compatible_egress_proxy_nodes AS node
    JOIN endpoint_credential_bindings AS binding
      ON binding.config_version_id = NEW.config_version_id
     AND binding.endpoint_id = NEW.endpoint_id
     AND binding.credential_id = NEW.credential_id
    WHERE node.config_version_id = NEW.config_version_id
      AND node.id = NEW.target_id
      AND node.upstream_id = binding.upstream_id
      AND node.pool_id IS NULL
 )
BEGIN
    SELECT RAISE(ABORT, 'compatible fixed proxy target is invalid');
END;

CREATE TRIGGER compatible_egress_binding_fixed_target_update
BEFORE UPDATE OF config_version_id, endpoint_id, credential_id, target_kind, target_id
ON compatible_egress_binding_profiles
WHEN NEW.target_kind = 'fixed_proxy'
 AND NOT EXISTS (
    SELECT 1
    FROM compatible_egress_proxy_nodes AS node
    JOIN endpoint_credential_bindings AS binding
      ON binding.config_version_id = NEW.config_version_id
     AND binding.endpoint_id = NEW.endpoint_id
     AND binding.credential_id = NEW.credential_id
    WHERE node.config_version_id = NEW.config_version_id
      AND node.id = NEW.target_id
      AND node.upstream_id = binding.upstream_id
      AND node.pool_id IS NULL
 )
BEGIN
    SELECT RAISE(ABORT, 'compatible fixed proxy target is invalid');
END;

CREATE TRIGGER compatible_egress_binding_pool_target_insert
BEFORE INSERT ON compatible_egress_binding_profiles
WHEN NEW.target_kind = 'proxy_pool'
 AND NOT EXISTS (
    SELECT 1
    FROM compatible_egress_proxy_pools AS pool
    JOIN endpoint_credential_bindings AS binding
      ON binding.config_version_id = NEW.config_version_id
     AND binding.endpoint_id = NEW.endpoint_id
     AND binding.credential_id = NEW.credential_id
    WHERE pool.config_version_id = NEW.config_version_id
      AND pool.id = NEW.target_id
      AND pool.upstream_id = binding.upstream_id
 )
BEGIN
    SELECT RAISE(ABORT, 'compatible proxy pool target is invalid');
END;

CREATE TRIGGER compatible_egress_binding_pool_target_update
BEFORE UPDATE OF config_version_id, endpoint_id, credential_id, target_kind, target_id
ON compatible_egress_binding_profiles
WHEN NEW.target_kind = 'proxy_pool'
 AND NOT EXISTS (
    SELECT 1
    FROM compatible_egress_proxy_pools AS pool
    JOIN endpoint_credential_bindings AS binding
      ON binding.config_version_id = NEW.config_version_id
     AND binding.endpoint_id = NEW.endpoint_id
     AND binding.credential_id = NEW.credential_id
    WHERE pool.config_version_id = NEW.config_version_id
      AND pool.id = NEW.target_id
      AND pool.upstream_id = binding.upstream_id
 )
BEGIN
    SELECT RAISE(ABORT, 'compatible proxy pool target is invalid');
END;

CREATE TRIGGER compatible_egress_proxy_node_reference_delete
BEFORE DELETE ON compatible_egress_proxy_nodes
WHEN EXISTS (
    SELECT 1 FROM config_versions WHERE id = OLD.config_version_id
 )
 AND EXISTS (
    SELECT 1 FROM upstreams
    WHERE config_version_id = OLD.config_version_id AND id = OLD.upstream_id
 )
 AND EXISTS (
    SELECT 1 FROM compatible_egress_binding_profiles
    WHERE config_version_id = OLD.config_version_id
      AND target_kind = 'fixed_proxy'
      AND target_id = OLD.id
 )
BEGIN
    SELECT RAISE(ABORT, 'compatible proxy node is still referenced');
END;

CREATE TRIGGER compatible_egress_proxy_pool_reference_delete
BEFORE DELETE ON compatible_egress_proxy_pools
WHEN EXISTS (
    SELECT 1 FROM config_versions WHERE id = OLD.config_version_id
 )
 AND EXISTS (
    SELECT 1 FROM upstreams
    WHERE config_version_id = OLD.config_version_id AND id = OLD.upstream_id
 )
 AND (
    EXISTS (
        SELECT 1 FROM compatible_egress_proxy_nodes
        WHERE config_version_id = OLD.config_version_id AND pool_id = OLD.id
    )
    OR EXISTS (
        SELECT 1 FROM compatible_egress_binding_profiles
        WHERE config_version_id = OLD.config_version_id
          AND target_kind = 'proxy_pool'
          AND target_id = OLD.id
    )
 )
BEGIN
    SELECT RAISE(ABORT, 'compatible proxy pool is still referenced');
END;

CREATE TRIGGER compatible_egress_proxy_node_reference_update
BEFORE UPDATE OF config_version_id, id, upstream_id, pool_id
ON compatible_egress_proxy_nodes
WHEN EXISTS (
    SELECT 1 FROM compatible_egress_binding_profiles
    WHERE config_version_id = OLD.config_version_id
      AND target_kind = 'fixed_proxy'
      AND target_id = OLD.id
 )
BEGIN
    SELECT RAISE(ABORT, 'compatible proxy node is still referenced');
END;

CREATE TRIGGER compatible_egress_proxy_pool_reference_update
BEFORE UPDATE OF config_version_id, id, upstream_id
ON compatible_egress_proxy_pools
WHEN EXISTS (
    SELECT 1 FROM compatible_egress_proxy_nodes
    WHERE config_version_id = OLD.config_version_id AND pool_id = OLD.id
 )
 OR EXISTS (
    SELECT 1 FROM compatible_egress_binding_profiles
    WHERE config_version_id = OLD.config_version_id
      AND target_kind = 'proxy_pool'
      AND target_id = OLD.id
 )
BEGIN
    SELECT RAISE(ABORT, 'compatible proxy pool is still referenced');
END;
