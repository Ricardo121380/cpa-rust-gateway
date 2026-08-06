# P12-10I-13 Web local expiry-cap receipt

Status: `LOCAL_PASS_PENDING_WEB_E2E`

## Boundary

This change resolves only the CPAR import-policy mismatch between a valid long-lived source Cookie
and CPAR's 90-day local Web session window. It does not bind the Web production adapter, obtain a
Statsig signature, send an upstream request, change a production graph, or modify a server.

No SSO, Cookie, endpoint, model, account identity, request/response value, or token fingerprint is
recorded here.

## Decision

The frozen grok2api reference imports Web SSO without an expiry field and treats the token as
usable until rejected upstream. Autoreg's browser artifact instead exposes a real Cookie expiry of
approximately 180 days. CPAR's 90-day limit is a local risk window, not an upstream maximum.

The governed migration boundary now persists an effective expiry of:

```text
min(source_expiry, observed_at + 90 days)
```

This can only shorten local authority. It cannot extend the source credential, revive an expired
credential, or allow a request after the upstream expires. The normal strict SSO importer remains
unchanged and continues to reject overlong input.

## Implementation

- The Web credential module performs strict duplicate/shape/timestamp validation before and after
  migration-only normalization.
- Expired, malformed, unsafe, duplicate, missing, non-positive, or overflowed inputs still fail
  closed.
- A source expiry within 90 days is retained byte-for-byte.
- A longer source expiry is canonicalized to the exact local maximum before the account credential
  is encrypted and persisted.
- Migration receipts expose only `capped_web_expiries`; no timestamp or credential value is logged.
- Build and Console migration behavior is unchanged.

## Verification

```text
cargo fmt --all -- --check
cargo test --locked -p provider-grok
cargo clippy --locked -p provider-grok --all-targets -- -D warnings
cargo clippy --locked -p gateway -- -D warnings
```

The provider suite passed all active tests. The new migration regression imports a synthetic
180-day source, records one capped Web expiry, opens the encrypted credential, and proves the
persisted effective expiry is exactly 90 days. The strict P9 credential regression separately
proves a 91-day direct import remains `InvalidTimestamp`.

## Remaining boundary

This is not a Web reverse-proxy success. The gateway intentionally leaves `grok.web.responses`
unbound because production Statsig/session transport orchestration has not yet been composed into
the runtime. P12-10I-14 must implement and review that data-plane binding before one signed artifact
can perform the required real CPAR base-URL/client-key Web E2E.
