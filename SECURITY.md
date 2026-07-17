# Security policy

## Secret handling

- Never commit API keys, OAuth tokens, cookies, private keys, production databases, raw provider logs, or unredacted request bodies.
- Put local credentials outside the repository and inject them through runtime configuration.
- Test fixtures must be synthetic or irreversibly redacted.
- The repository uses `scripts/secret-scan.sh` in the pre-commit hook and CI. It reports only file names, never matching values.
- Bypassing the hook is not an accepted workflow. A false positive must be resolved by changing the fixture or improving the scanner in a reviewed task.

## Reporting

Do not open a public issue containing a credential or production trace. Revoke an exposed credential first, then record only a redacted incident summary.
