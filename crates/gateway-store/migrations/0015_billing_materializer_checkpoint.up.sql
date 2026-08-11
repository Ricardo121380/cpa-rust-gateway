CREATE TABLE billing_materializer_checkpoints (
    materializer_id TEXT PRIMARY KEY
        CHECK(length(trim(materializer_id)) BETWEEN 1 AND 128),
    event_ordinal INTEGER NOT NULL CHECK(event_ordinal >= 0),
    updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms >= 0)
) STRICT;
