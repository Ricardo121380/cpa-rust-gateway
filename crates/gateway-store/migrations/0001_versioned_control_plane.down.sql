DROP TABLE endpoint_credential_bindings;
DROP TABLE upstream_credentials;
DROP TABLE upstream_endpoints;
DROP TABLE upstreams;
UPDATE config_versions SET parent_id = NULL;
DROP TABLE config_versions;
