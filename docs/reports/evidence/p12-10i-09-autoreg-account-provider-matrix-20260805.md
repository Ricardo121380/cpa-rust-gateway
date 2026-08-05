# P12-10I-09 Autoreg account provider matrix

Status: `BLOCKED_WITH_EVIDENCE`

## Scope

Test the single account imported by `autoreg-task55-prod-20260805` across CPAR's native
Build, Console, and Web provider boundaries. The account was imported as `grok_build`; no
credential value, account identity, endpoint value, request/response body, token fingerprint,
or password is recorded here.

## Precondition and database health

- The production row for this batch is exactly one enabled, active Build account with the
  recorded priority/weight/concurrency metadata and an encrypted credential.
- The production database returned `quick_check=ok`.
- The active Config Version remains `p12-09-codex-production-v2`.
- The active graph has two enabled upstream endpoints, zero enabled native Grok endpoints, and
  zero Grok model routes.

## Results

### Build

The production database was copied with SQLite online backup into a root-only temporary
directory. The deployment credential directory was copied only for the duration of the probe;
the production database was not opened for mutation.

The single selected account passed the native probe:

```text
native_grok_probe=PASS provider=grok_build account_attributed=true health=available quota=available canonical_complete=true chat=true responses=true messages=true
```

This is a real upstream request from the isolated provider probe. It proves that this imported
Build OAuth can be selected, leased, authenticated, and projected into all three CPAR protocol
forms for the probe response. It does not prove public CPAR HTTP availability because the active
production graph has no Grok route.

### Console shape safety

The same Build credential payload was sent through a separate database copy while explicitly
declared as `grok_console`. The importer rejected it before any account transaction:

```text
console_shape_rc=1 result=PASS
accepted_accounts=0 rejected_records=1
temporary_batch_accounts=0
quick_check=ok
```

No Console upstream request was sent. A Build OAuth is not accepted as a Console SSO credential.

### Web shape safety

The same Build credential payload was sent through another database copy while explicitly declared
as `grok_web`. The importer rejected it before any account transaction:

```text
web_shape_rc=1 result=PASS
accepted_accounts=0 rejected_records=1
temporary_batch_accounts=0
quick_check=ok
```

No Web session request was sent. A Build OAuth is not accepted as a Web SSO-cookie credential.

## Cleanup and review

- All temporary database copies and root-only credential directories were removed after each
  probe.
- Production service, active Config Version, public listeners, Caddy/DNS, old CPA, grok2api, and
  CC Switch were not changed.
- The result is reviewed as a provider-isolation check: Build is `PASS` at the native provider
  probe; Console and Web are `PASS` for fail-closed type rejection, not successful provider use.
- Public CPAR curl was intentionally not attempted for this batch because route preflight would
  stop at `grok_route_missing`; sending a request would not test the account and would violate the
  current no-route boundary.

## Remaining boundary

To claim Console or Web usability, CPAR needs an independently valid Console SSO or Web session
credential and a separately approved native route in an isolated Config Version. This Build account
must not be cross-used or used as an implicit fallback for either provider.
