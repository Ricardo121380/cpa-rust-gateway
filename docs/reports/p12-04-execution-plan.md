# P12-04 Staging execution plan

| Field | Value |
|---|---|
| Plan version | `v1.48` |
| Task | `P12-04` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` — the isolated loopback Staging instance is active but deliberately disabled at boot; its acceptance receipt is [P12-04 Staging receipt](p12-04-staging-receipt.md). |
| Preconditions | P12-02 and P12-03 are `LOCAL_PASS_PENDING_PHASE_GATE`. The pre-`serve` P12-01 artifact remains rejected. The independently verified replacement is GitHub run `30142690538`, revision `111f60a416fd0a6b4a6314bac8ff32b0074cdca7`, private artifact ID `8615022248`. |
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
   signature identity/issuer/revision, manifest and receipt digests, SBOM redaction, and Linux
   amd64 non-root OCI structure. The local macOS host cannot execute the Linux binary and its
   Docker daemon is unavailable; do not start or change Docker. The Linux `gateway serve --help`
   check is therefore deliberately deferred to the isolated server staging path.
3. Read-only inspect the server's Linux architecture, systemd version, unused loopback ports, and
   proposed staging paths. The preflight has passed. Stage only the verified binary by digest in
   the new release path, run `gateway serve --help`, and run `systemd-analyze verify` against the
   exact repaired Unit (SHA-256 `f40cf0e55116360fe8372240131f4fa69aea5e6692f0ec64a21a9d859397063d`)
   while it is still uninstalled. Stop on a digest, path, port, ownership, capacity, help, or
   Unit-hardening failure.
4. Only then create the dedicated unprivileged account, five root-only credential files, systemd
   Unit, and systemd-owned state/runtime/log directories. Reload systemd and start only the new
   loopback Staging Unit. The signed binary remains eligible because the P12-02 repair changes
   only the Unit, checker, regression harness, and documentation—not the signed binary inputs.
5. Verify the two listener boundaries, readiness, protected management admission, service owner,
   unit hardening, resource state, and value-free journal summary. Preserve an independent
   receipt, review, local/remote verification evidence, and exact staging rollback procedure.

## Completed outcome

The replacement private artifact was independently reverified and staged by its manifest digest.
The Linux binary's `serve --help` and the exact repaired Unit's systemd `255` verification passed
before installation.  The new service has only the two declared loopback listeners and has not
been enabled at boot.  See the receipt for runtime metadata, endpoint admission, credential
metadata, the two harmless procedure corrections that were fail-closed and rolled back, and the
exact new-instance-only rollback procedure.

## Stop conditions and rollback

- The P12-01 pre-`serve` artifact, an unsigned local build, any artifact identity/digest mismatch,
  failed Rekor/signature verification, a non-amd64 host, public/non-loopback listener, staging path
  collision, credentials outside the declared root-only files, a Unit verification failure, or any
  incumbent-service change is a hard stop.
- Before the new Unit is started, rollback is removal of only its new files. After start, rollback
  is `systemctl disable --now` for the staging Unit plus removal of its new release/Unit/state
  paths after preserving its value-free receipt. It never stops, restarts, replaces, or restores
  the incumbent CPA container.
