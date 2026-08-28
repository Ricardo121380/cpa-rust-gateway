ALTER TABLE grok_accounts ADD COLUMN quota_sync_due_at_ms INTEGER
    CHECK (quota_sync_due_at_ms IS NULL OR quota_sync_due_at_ms >= 0);
ALTER TABLE grok_accounts ADD COLUMN last_quota_sync_at_ms INTEGER
    CHECK (last_quota_sync_at_ms IS NULL OR last_quota_sync_at_ms >= 0);
ALTER TABLE grok_accounts ADD COLUMN quota_sync_failure_count INTEGER NOT NULL DEFAULT 0
    CHECK (quota_sync_failure_count >= 0);
ALTER TABLE grok_accounts ADD COLUMN worker_claim_kind TEXT
    CHECK (worker_claim_kind IS NULL OR worker_claim_kind IN ('refresh', 'quota'));
ALTER TABLE grok_accounts ADD COLUMN worker_claim_id TEXT
    CHECK (
        worker_claim_id IS NULL OR (
            length(trim(worker_claim_id)) BETWEEN 1 AND 128
            AND length(CAST(worker_claim_id AS BLOB)) <= 128
        )
    );
ALTER TABLE grok_accounts ADD COLUMN worker_claim_expires_at_ms INTEGER
    CHECK (worker_claim_expires_at_ms IS NULL OR worker_claim_expires_at_ms >= 0)
    CHECK (
        (
            worker_claim_kind IS NULL
            AND worker_claim_id IS NULL
            AND worker_claim_expires_at_ms IS NULL
        ) OR (
            worker_claim_kind IS NOT NULL
            AND worker_claim_id IS NOT NULL
            AND worker_claim_expires_at_ms IS NOT NULL
        )
    );

CREATE INDEX grok_accounts_worker_due_idx
    ON grok_accounts (
        enabled,
        auth_status,
        worker_claim_expires_at_ms,
        refresh_due_at_ms,
        quota_sync_due_at_ms,
        id
    );

CREATE TABLE grok_account_quota_windows (
    account_id TEXT NOT NULL REFERENCES grok_accounts(id) ON DELETE CASCADE,
    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('account', 'model')),
    model_label TEXT NOT NULL CHECK (
        length(CAST(model_label AS BLOB)) <= 256
        AND (
            (scope_kind = 'account' AND model_label = '')
            OR (scope_kind = 'model' AND length(trim(model_label)) BETWEEN 1 AND 256)
        )
    ),
    window_label TEXT NOT NULL CHECK (
        length(trim(window_label)) BETWEEN 1 AND 64
        AND length(CAST(window_label AS BLOB)) <= 64
    ),
    quota_limit INTEGER CHECK (quota_limit IS NULL OR quota_limit >= 0),
    quota_remaining INTEGER CHECK (
        quota_remaining IS NULL OR quota_remaining >= 0
    ),
    reset_at_ms INTEGER CHECK (reset_at_ms IS NULL OR reset_at_ms >= 0),
    source TEXT NOT NULL CHECK (source IN ('billing', 'rest', 'grpc', 'estimated')),
    confidence TEXT NOT NULL CHECK (
        confidence IN ('authoritative', 'observed', 'estimated')
    ),
    observed_at_ms INTEGER NOT NULL CHECK (observed_at_ms >= 0),
    CHECK (
        quota_limit IS NULL OR quota_remaining IS NULL OR quota_remaining <= quota_limit
    ),
    PRIMARY KEY (account_id, scope_kind, model_label, window_label)
) STRICT;
