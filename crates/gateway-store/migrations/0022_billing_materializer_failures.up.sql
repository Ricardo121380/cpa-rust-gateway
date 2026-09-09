CREATE TABLE billing_materializer_failures (
    materializer_id TEXT NOT NULL CHECK(length(trim(materializer_id)) BETWEEN 1 AND 128),
    event_ordinal INTEGER NOT NULL CHECK(event_ordinal > 0),
    reason TEXT NOT NULL CHECK(reason IN ('invalid_lineage', 'invalid_timestamp', 'pricing_overflow', 'invalid_event')),
    first_seen_at_ms INTEGER NOT NULL CHECK(first_seen_at_ms >= 0),
    last_attempt_at_ms INTEGER NOT NULL CHECK(last_attempt_at_ms >= first_seen_at_ms),
    attempts INTEGER NOT NULL CHECK(attempts > 0),
    resolved_at_ms INTEGER CHECK(resolved_at_ms IS NULL OR resolved_at_ms >= first_seen_at_ms),
    PRIMARY KEY(materializer_id, event_ordinal)
) STRICT;
CREATE INDEX billing_materializer_failures_due
    ON billing_materializer_failures(materializer_id, resolved_at_ms, last_attempt_at_ms, event_ordinal);
