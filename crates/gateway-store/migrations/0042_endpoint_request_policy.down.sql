-- Only semantically empty policies can be represented by the previous schema.
CREATE TEMP TABLE issue71_downgrade_guard (ok INTEGER CHECK (ok = 1));
INSERT INTO issue71_downgrade_guard SELECT CASE WHEN EXISTS (
    SELECT 1 FROM upstream_endpoints, json_each(request_policy_json) AS item
    WHERE NOT (
        (item.key = 'rules' AND item.type = 'array' AND json_array_length(item.value) = 0)
        OR (item.key = 'headers' AND item.type = 'object'
            AND NOT EXISTS (SELECT 1 FROM json_each(item.value)))
    )
) THEN 0 ELSE 1 END;
DROP TABLE issue71_downgrade_guard;
ALTER TABLE upstream_endpoints DROP COLUMN request_policy_json;
