CREATE TABLE routing_price_policies (
    config_version_id TEXT PRIMARY KEY
        REFERENCES config_versions (id) ON DELETE CASCADE,
    catalog_version_id TEXT NOT NULL
        REFERENCES billing_price_catalog_versions (catalog_version_id) ON DELETE RESTRICT
        CHECK (
            length(trim(catalog_version_id)) BETWEEN 1 AND 128
            AND length(CAST(catalog_version_id AS BLOB)) <= 128
        ),
    comparison TEXT NOT NULL CHECK (comparison IN ('rate_dominance_v1'))
) STRICT;
