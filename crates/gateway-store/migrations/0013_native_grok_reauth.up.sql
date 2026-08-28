CREATE TABLE grok_account_reauth_state (
    account_id TEXT PRIMARY KEY REFERENCES grok_accounts(id) ON DELETE CASCADE,
    next_attempt_at_ms INTEGER CHECK (
        next_attempt_at_ms IS NULL OR next_attempt_at_ms >= 0
    ),
    failure_count INTEGER NOT NULL DEFAULT 0 CHECK (failure_count >= 0),
    operator_required INTEGER NOT NULL DEFAULT 0 CHECK (operator_required IN (0, 1)),
    claim_id TEXT CHECK (
        claim_id IS NULL OR (
            length(trim(claim_id)) BETWEEN 1 AND 128
            AND length(CAST(claim_id AS BLOB)) <= 128
        )
    ),
    claim_expires_at_ms INTEGER CHECK (
        claim_expires_at_ms IS NULL OR claim_expires_at_ms >= 0
    ),
    CHECK (
        (claim_id IS NULL AND claim_expires_at_ms IS NULL)
        OR (claim_id IS NOT NULL AND claim_expires_at_ms IS NOT NULL)
    )
) STRICT;

INSERT INTO grok_account_reauth_state (account_id)
SELECT id FROM grok_accounts WHERE auth_status = 'reauth_required';

CREATE INDEX grok_account_reauth_due_idx
    ON grok_account_reauth_state (
        operator_required, next_attempt_at_ms, claim_expires_at_ms, account_id
    );
