CREATE TABLE billing_catalog_reviews (
    catalog_version_id TEXT PRIMARY KEY REFERENCES billing_price_catalog_versions(catalog_version_id),
    review_json TEXT NOT NULL CHECK (length(CAST(review_json AS BLOB)) <= 524288 AND json_valid(review_json))
) STRICT;
