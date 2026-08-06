# P12-10I-15 grok2api latest-source Console receipt

Status: `BLOCKED_WITH_EVIDENCE`

## Source update check

- Remote grok2api repository was fetched on the Jakarta host.
- Latest reachable source revision: `9b7e5f8`.
- The provider reverse/grok source tree has no commits after the already vendored revision.
- The server working tree contains parent deployment changes, but no newer tracked changes inside
  the vendored grok2api provider source to port into CPAR.
- Therefore no CPAR code was overwritten or blindly copied. CPAR's existing Console adapter remains
  the independently typed implementation for its Console provider boundary; grok2api's current
  reverse app-chat path is a separate Web boundary.

## Isolated Console probe

One account row from the grok2api account database was selected without recording its token,
identity, or source values. The source row was already marked `expired`. It was wrapped into CPAR's
canonical Console credential envelope and sent through the root-only importer pipe into a fresh
isolated CPAR database. The production database and route graph were not touched.

The native Console probe returned:

```text
native_grok_probe=FAIL stage=start code=CredentialUnauthorized scope=Credential
```

The probe stopped before a successful upstream response. No public route was published, and the
temporary database, credential copy, and binary were securely removed after the check.

## Verdict

The current grok2api source provides no new tracked Console implementation to replicate, and its
available account database has no active Console credential. The Console channel cannot be marked
passed from this run. A valid, non-expired Console SSO credential is required for a meaningful CPAR
Console E2E; retrying the expired row would not add evidence.
