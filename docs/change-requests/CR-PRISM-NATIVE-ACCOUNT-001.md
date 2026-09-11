# Native Grok enrollment and Device OAuth

2026-09-11. Implemented locally; no production deployment.

## Contract and actual storage

- `listNativeAccounts`: global protected native metadata inventory with SQL filtering, 1–100
  item pages and opaque cursor. It reads GrokAccountPoolStore rather than runtime bindings or
  upstream_credentials. Build/Console/Web, disabled and currently unbound identities are visible.
- `importNativeAccount`: one bounded, operator-named native account, using the existing atomic
  migration/import parser. Build accepts supported absolute-expiry JSON; Console accepts SSO
  credential JSON; Web accepts the strict cookie/lineage/expiry envelope. Account name is the
  explicit import identity and unique batch name; it is not inferred from email or token text.
  Conflicting names/identities are rejected, never overwritten or automatically replayed.
- `startNativeAccountAuthorization`, `pollNativeAccountAuthorization`,
  `cancelNativeAccountAuthorization`: Grok Build Device Grant through the existing provider
  library. Start requires a name and optionally an exact account ID/revision for reauthorization.
  The server retains device_code and token material; only verification URI, user code, session
  ID, state, expiry and next allowed polling time reach Prism. Polling honors the provider's
  interval/slow_down and only persists an actual granted credential.

Native state is deliberately global, separate from configuration-draft CRUD. These five
operations do not falsely declare X-Config-Version or If-Match. Browser management admission and
CSRF protection remain mandatory. The deployment root injects the same database/key material
used by the native runtime. Account imports do not silently publish a graph or restart serving.
The UI says that newly saved authorization is used after configuration reload.

## Correctness and resource bounds

- Schema 24 adds an account-specific inventory generation with triggers on native metadata
  changes. Unrelated gateway requests/config writes do not invalidate native pagination.
- Reauthorization checks the stored Build account, exact revision and matching non-empty JWT
  `sub` from old/new material. Opaque or otherwise unverifiable identity evidence is rejected.
  The new credential is re-sealed using the original account AAD; ID, provider, source identity,
  binding/history references and enable state remain. Old worker/reauth claims are cleared in the
  same transaction; a dedicated native authorization event records the new revision.
- Session count is bounded to 32, lifetime capped at 15 minutes and expired sessions are pruned
  on new starts. Terminal/cancelled/expired sessions release the secret-bearing device poller.
  Terminal safe results remain until expiry so repeated polls cannot duplicate an account.
- HTTP OAuth only posts to the library's fixed HTTPS auth endpoints, rejects redirects, caps
  responses at 64 KiB and has a 30-second timeout. No refresh-first fallback or credential export
  is introduced. The current transport is direct; no global proxy configuration is changed.
- Network/storage work runs under existing bounded blocking admission. A failed request is not
  automatically replayed. Browser polling stops on errors or terminal states, and account errors
  do not raise an unrelated configuration-conflict banner.

## Validation and actual external boundary

Seven real SQLite/HTTP inventory/import regressions passed. Two device-workflow regressions
exercise first grant, same-account reauthorization, wrong-account/stale revision rejection,
cancellation and expiry. Seven existing native migration/rollback/parser tests passed.
Seven browser tests cover ordinary/native imports, explicit synthetic provider consent, stable
account ID on reauthorization and desktop/mobile entries. Schema up/down and the 122-operation
contract/four-file double-build gate passed. Clippy checked the gateway composition, HTTP and
provider crates.

A clean real local gateway was started. After moving the interaction to the user's requested
Chrome browser, first authorization and same-account reauthorization both completed against
Grok's actual service. The account remained unique, ID/import identity stayed unchanged, revision
advanced from 0 to 1, ciphertext changed and an authorization audit was written. After rebuilding
and restarting the independent gateway, the encrypted row and API/UI readback remained intact.
No provider inference request or production CPAR change was part of this authorization test.
The live receipt is docs/reports/evidence/prism-grok-auth-live-20260911.json.

All nine credential-entry families are connected. This does not mark the wider functional plan
complete: Codex/Claude/Kiro first OAuth wiring, full batch preview/commit, provider onboarding,
runtime generation switching and later model/request/settings work remain tracked separately.
