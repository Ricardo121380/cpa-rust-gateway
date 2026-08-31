CREATE TABLE grok_account_entitlements (
    account_id TEXT PRIMARY KEY REFERENCES grok_accounts(id) ON DELETE CASCADE,
    domain TEXT NOT NULL CHECK (domain IN ('grok_build', 'grok_web')),
    tier TEXT NOT NULL,
    source TEXT NOT NULL CHECK (
        source IN ('provider_subscription', 'signed_token', 'imported_metadata')
    ),
    confidence TEXT NOT NULL CHECK (
        confidence IN ('authoritative', 'derived', 'declared')
    ),
    observed_at_ms INTEGER NOT NULL CHECK (observed_at_ms >= 0),
    CHECK (
        (domain = 'grok_build' AND tier IN ('unknown', 'free', 'supergrok', 'heavy'))
        OR (domain = 'grok_web' AND tier IN ('unknown', 'basic', 'super', 'heavy'))
    ),
    CHECK (
        (source = 'provider_subscription' AND confidence = 'authoritative')
        OR (source = 'signed_token' AND confidence = 'derived')
        OR (source = 'imported_metadata' AND confidence = 'declared')
    )
) STRICT;

CREATE INDEX grok_account_entitlements_by_domain_tier
ON grok_account_entitlements(domain, tier, account_id);

CREATE TRIGGER grok_account_entitlements_provider_insert
BEFORE INSERT ON grok_account_entitlements
WHEN NOT EXISTS (
    SELECT 1
    FROM grok_accounts AS account
    WHERE account.id = NEW.account_id
      AND (
          (account.provider = 'build' AND NEW.domain = 'grok_build')
          OR (account.provider = 'web' AND NEW.domain = 'grok_web')
      )
)
BEGIN
    SELECT RAISE(ABORT, 'native Grok entitlement domain does not match account provider');
END;

CREATE TRIGGER grok_account_entitlements_provider_update
BEFORE UPDATE OF account_id, domain ON grok_account_entitlements
WHEN NOT EXISTS (
    SELECT 1
    FROM grok_accounts AS account
    WHERE account.id = NEW.account_id
      AND (
          (account.provider = 'build' AND NEW.domain = 'grok_build')
          OR (account.provider = 'web' AND NEW.domain = 'grok_web')
      )
)
BEGIN
    SELECT RAISE(ABORT, 'native Grok entitlement domain does not match account provider');
END;
