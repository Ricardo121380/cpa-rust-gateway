# P12-10I-13 Review

Decision: `PASS_WITH_LIVE_E2E_PENDING`

## Findings

- **No P0/P1 finding.** The effective expiry is the minimum of source and local authority. The
  change never expands the source lifetime or CPAR's 90-day maximum.
- **Strict admission remains strict.** `GrokWebCredential::import_sso_json` was not relaxed and a
  direct 91-day import still fails. Only the bounded migration adapter can normalize a valid future
  source expiry.
- **The reduction is observable.** A value-free receipt count prevents the policy change from being
  silent while avoiding timestamps, identities, or credential material.
- **Persisted authority is actually reduced.** The encrypted account stores normalized JSON, not
  the original 180-day envelope, so later runtime parsing cannot recover or use the longer expiry.
- **No false live claim.** The change proves import behavior only. Web production adapter binding,
  Statsig/session orchestration, and CPAR HTTP E2E remain pending.

## Evidence reviewed

- Migration-only normalizer and its pre/post strict validation.
- Receipt propagation through the public administrative CLI output.
- Synthetic 180-day-to-90-day persistence regression.
- Strict direct-import 91-day rejection regression.
- Full active provider-grok test suite and focused provider/gateway Clippy.
- Diff/whitespace review and unchanged production/server state.

## Conclusion

Accept P12-10I-13 as `LOCAL_PASS_PENDING_WEB_E2E`. Batch its signed artifact and server delivery
with P12-10I-14 so the next remote cycle proves the actual CPAR Web data plane instead of spending a
release solely on an internal import result.
