# P12-04 Staging execution plan

| Field | Value |
|---|---|
| Plan version | `v1.48` |
| Task | `P12-04` |
| Status | `IN_PROGRESS` |
| Preconditions | P12-02 deployment envelope and P12-03 incumbent backup are `LOCAL_PASS_PENDING_PHASE_GATE`. The previously accepted artifact is rejected for this task because it pre-dates `gateway serve`. |
| Approved remote exception | `CR-P12-04-001`: one current-revision private GitHub artifact, OIDC keyless manifest signature, and public Rekor inclusion before any server write. |

## Staging boundary

P12-04 may deploy exactly one new, isolated gateway process after the signed candidate has passed
independent verification. It must use its own service identity, release path, systemd-created
state/runtime/log directories, credential directory, and two non-conflicting loopback ports. It
cannot reuse `/opt/example-legacy-gateway/cpa`, the incumbent container, its configuration, its credentials,
or any existing Caddy/Cloudflare/DNS/traffic path.

The first listener is limited to P12-02 readiness (`HEAD /`, `GET /healthz`); the protected
management listener remains loopback-only. P12-04 does not add a Provider, Client Key, Route,
Upstream, inference listener, proxy exposure, test Credential, HTTP Provider request, or traffic
mirroring. Those remain P12-05/P12-06 work.

## Ordered execution

1. Record the `CR-P12-04-001` early remote exception, push only the reviewed P12 branch commits,
   and dispatch the pinned `release-artifact` workflow at that exact remote SHA. Do not create a
   PR, tag, GitHub Release, registry image, or server-side change.
2. Download the resulting private artifact to a disposable local verification directory. Check
   signature identity/issuer/revision, manifest and receipt digests, SBOM redaction, Linux amd64
   non-root OCI structure, and `gateway serve --help`. Stop if the artifact differs from the
   pushed SHA or any check fails.
3. Read-only inspect the server's Linux architecture, systemd version, unused loopback ports, and
   proposed staging paths. Verify `systemd-analyze verify` against the exact candidate Unit before
   installation. Stop on a path, port, account, ownership, capacity, or Unit-hardening conflict.
4. Only then create the dedicated unprivileged account, root-only credentials, release directory,
   systemd Unit, and systemd-owned state/runtime/log directories. Install the verified binary by
   digest, reload systemd, and start only the new loopback Staging Unit.
5. Verify the two listener boundaries, readiness, protected management admission, service owner,
   unit hardening, resource state, and value-free journal summary. Preserve an independent
   receipt, review, local/remote verification evidence, and exact staging rollback procedure.

## Stop conditions and rollback

- The P12-01 pre-`serve` artifact, an unsigned local build, any artifact identity/digest mismatch,
  failed Rekor/signature verification, a non-amd64 host, public/non-loopback listener, staging path
  collision, credentials outside the declared root-only files, a Unit verification failure, or any
  incumbent-service change is a hard stop.
- Before the new Unit is started, rollback is removal of only its new files. After start, rollback
  is `systemctl disable --now` for the staging Unit plus removal of its new release/Unit/state
  paths after preserving its value-free receipt. It never stops, restarts, replaces, or restores
  the incumbent CPA container.
