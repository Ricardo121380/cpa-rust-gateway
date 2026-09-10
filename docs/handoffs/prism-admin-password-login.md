# Prism administrator login

The user approved username/password login, the existing V4 visual direction, and a random
initial `admin` password delivered through a private local file. This supersedes the old
Key/CSRF form in the V4 and domain handoffs. It does not change the CLI credential interface.

The design is saved in the existing OpenDesign project `prism-gateway-console-redesign-a735`
as `prism-v4-admin-login.html`. Codex authored it, used OpenDesign MCP `create_artifact` and
`get_artifact`, and checked its rendered preview. No model generator or external model CLI.
The implementation uses the existing React/GlassSurface/V4 tokens; prototype scripts/styles
are not copied into the production entry. The page has a brand, title, two fields and a primary
button. Necessary error and first-password messages are conditional. Settings shows the account
and session expiry; rendering/build explanations are behind a disclosure.

References: [grok2api login](https://github.com/chenyme/grok2api/blob/main/frontend/src/features/auth/login-page.tsx)
and [CPA Manager Plus login](https://github.com/seakee/CPA-Manager-Plus/blob/main/apps/web/src/features/login/LoginPage.tsx).
Grok2api's account/password workflow informed the interaction; CPA Manager Plus still uses an
Admin Key in its normal manager entry. Prism implements the user's explicit password requirement.

## First initialization

Run the new binary locally on the target host as the service account, before switching traffic:

```sh
gateway admin-login init --state-dir /absolute/state-directory --password-file /private/new-file
```

The username defaults to `admin`; `--username` accepts a 1–64 character ASCII account name.
The command creates the owner-only `admin-auth.sqlite3` and a new mode-0600 password file.
It never prints a password or overwrites an account/file. An existing store is required for
web login; there is no public registration, default shared password or automatic reset.

The initial login only opens the password-change form. Enter the initial password and a new
12–128 character password twice, then sign in again. The browser supports password managers.
Passwords are never trimmed. Updating a password revokes all existing administrator sessions.
Refresh, expiry and sign-out clear the browser's memory-only session and query/mutation caches.
Settings allows later password changes and sign-out.

`admin-auth.sqlite3` is authentication state, outside configuration export/restore and outside
control database schema 22. Back it up through the operator's private credential procedure,
never through the public management backup API. Binary rollback preserves the file; a rollback
to the previous release restores that release's key login without changing the stored password.
No automatic deletion/reset of authentication state is part of rollback.

## Boundaries and validation

See [the accepted contract change](../change-requests/prism-admin-password-login.md).
Argon2id: [RustCrypto API](https://docs.rs/argon2/0.5.3/argon2/) and
[OWASP password storage](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html).
The server shares its rate limit and 32-session cap across workers; password/SQLite work stays
on bounded blocking tasks. Session CSRF is independent of the legacy CLI CSRF. Existing peer,
Origin, CSP, separate data/management listeners and four-asset embedding remain enforced.

Reproduce with synthetic state only:

```sh
cargo build -p gateway --bin gateway
python3 scripts/test-admin-login.py --binary target/debug/gateway --browser-output output/admin-login-browser --report output/admin-login.json
```

This covers real serve/bootstrap/login/mandatory change/CSRF/draft write/read/logout/restart,
three viewport sizes and light/dark browser renders. No Provider request is made. The older
V4 acceptance harness now bootstraps an administrator before running its existing browser flows.
