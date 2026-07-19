CREATE TABLE egress_policies (
    config_version_id TEXT NOT NULL CHECK (length(trim(config_version_id)) BETWEEN 1 AND 128),
    id TEXT NOT NULL CHECK (length(trim(id)) BETWEEN 1 AND 128),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 256),
    allowed_schemes_json TEXT NOT NULL CHECK (json_valid(allowed_schemes_json) AND json_type(allowed_schemes_json) = 'array'),
    allowed_hosts_json TEXT NOT NULL CHECK (json_valid(allowed_hosts_json) AND json_type(allowed_hosts_json) = 'array'),
    allowed_ports_json TEXT NOT NULL CHECK (json_valid(allowed_ports_json) AND json_type(allowed_ports_json) = 'array'),
    allowed_cidrs_json TEXT NOT NULL CHECK (json_valid(allowed_cidrs_json) AND json_type(allowed_cidrs_json) = 'array'),
    redirect_mode TEXT NOT NULL CHECK (redirect_mode IN ('deny', 'same_origin', 'revalidate')),
    max_redirects INTEGER NOT NULL CHECK (
        (redirect_mode = 'deny' AND max_redirects = 0)
        OR (redirect_mode IN ('same_origin', 'revalidate') AND max_redirects BETWEEN 1 AND 10)
    ),
    PRIMARY KEY (config_version_id, id),
    UNIQUE (config_version_id, name),
    FOREIGN KEY (config_version_id) REFERENCES config_versions(id) ON DELETE CASCADE
) STRICT;

INSERT INTO egress_policies (
    config_version_id, id, name, allowed_schemes_json, allowed_hosts_json,
    allowed_ports_json, allowed_cidrs_json, redirect_mode, max_redirects
)
SELECT
    config_version_id,
    egress_policy_id,
    'legacy-unconfigured-' || egress_policy_id,
    '[]',
    '[]',
    '[]',
    '[]',
    'deny',
    0
FROM upstreams
WHERE egress_policy_id IS NOT NULL
GROUP BY config_version_id, egress_policy_id;

CREATE TRIGGER upstreams_egress_policy_reference_insert
BEFORE INSERT ON upstreams
WHEN NEW.egress_policy_id IS NOT NULL
 AND NOT EXISTS (
    SELECT 1
    FROM egress_policies
    WHERE config_version_id = NEW.config_version_id
      AND id = NEW.egress_policy_id
 )
BEGIN
    SELECT RAISE(ABORT, 'upstream egress policy reference is invalid');
END;

CREATE TRIGGER upstreams_egress_policy_reference_update
BEFORE UPDATE OF egress_policy_id, config_version_id ON upstreams
WHEN NEW.egress_policy_id IS NOT NULL
 AND NOT EXISTS (
    SELECT 1
    FROM egress_policies
    WHERE config_version_id = NEW.config_version_id
      AND id = NEW.egress_policy_id
 )
BEGIN
    SELECT RAISE(ABORT, 'upstream egress policy reference is invalid');
END;

CREATE TRIGGER egress_policies_reference_delete
BEFORE DELETE ON egress_policies
BEGIN
    UPDATE upstreams
    SET egress_policy_id = NULL
    WHERE config_version_id = OLD.config_version_id
      AND egress_policy_id = OLD.id;
END;

CREATE TRIGGER egress_policies_reference_key_update
BEFORE UPDATE OF id, config_version_id ON egress_policies
WHEN (NEW.id != OLD.id OR NEW.config_version_id != OLD.config_version_id)
 AND EXISTS (
    SELECT 1
    FROM upstreams
    WHERE config_version_id = OLD.config_version_id
      AND egress_policy_id = OLD.id
)
BEGIN
    SELECT RAISE(ABORT, 'egress policy identity is still referenced by an upstream');
END;

UPDATE upstreams
SET egress_policy_id = egress_policy_id
WHERE egress_policy_id IS NOT NULL;
