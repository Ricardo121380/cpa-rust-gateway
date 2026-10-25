ALTER TABLE upstream_endpoints ADD COLUMN request_policy_json TEXT NOT NULL DEFAULT '{}'
    CHECK (json_valid(request_policy_json) AND json_type(request_policy_json) = 'object');
