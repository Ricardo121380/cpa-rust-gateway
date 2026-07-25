# P12-02 deployment envelope execution plan

| Field | Value |
|---|---|
| Plan version | `v1.48` |
| Task | `P12-02` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` — implementation, local Full gate, independent review, and real-Linux `systemd-analyze verify` repair validation are complete. P12-04 repeats it against the exact Unit after staging the verified binary, before installing the Unit. |
| Scope | Explicit Linux service envelope: systemd unit, credential hand-off, state/log directories, resource limits, and the minimum process composition needed for those assets to start safely. |
| Preconditions | P12-01's accepted private revision-bound artifact; no server state, provider credential, route, Caddy, Cloudflare, or production traffic is in scope. |
| Approved change | `CR-P12-02-001`: add a minimal `gateway serve` composition entry because the prior artifact exposed only the transport-free `gateway admin` CLI. |

## Goal and success criteria

P12-02 must produce a reviewable Linux deployment envelope without pretending that the gateway is
ready for provider traffic.  The resulting unit must pass `systemd-analyze verify` on Linux, run
as a dedicated unprivileged account, accept credentials only through systemd's read-only
credential directory, own only its declared state/runtime/log directories, and set explicit
restart, file-descriptor, memory, CPU, task, and sandbox limits.

The process entry must bind two explicit, distinct **loopback** listeners:

1. The data listener serves only `HEAD /` and `GET /healthz` in P12-02.  It does not construct a
   Client-Key authenticator, load a RouteSnapshot, select a Provider, read an Endpoint, or accept
   an inference route.
2. The management listener composes the already implemented P10 management API and embedded UI
   with its separate `ManagementKey`, P10 network/browser policy, versioned SQLite services, and
   encrypted-backup facade.  It neither changes the management contract nor makes the management
   listener publicly reachable.

That division gives P12-04 a truthful loopback staging-health target and gives P12-05 an existing
private management surface for controlled configuration work.  P12-05 must add the separately
reviewed data-plane runtime composition before any Responses, Messages, Tools, model, or Explain
claim is made.

## Design and implementation steps

1. Refactor the existing Actix configuration into an explicit data-plane configuration and a
   small public readiness configuration.  Keep management routes out of the data-plane
   configuration; P10's protected management scopes are mounted only on the management listener.
2. Add `gateway serve` with strict structured options for two loopback socket addresses, a
   systemd-owned state directory, and the passed credential directory.  Reject duplicate,
   unknown, relative, non-loopback, zero-port, or identical-listener inputs before binding.
3. Read only five direct credentials from the supplied directory: the namespaced management key,
   independent management CSRF token, Master Key, backup key, and Client-Key pepper. Admission
   rejects symlinks, special files, malformed/oversized material, absent required entries, and
   errors without rendering a path or value. Secret buffers are zeroized after their typed owner
   takes over.
4. Construct the P10 resource, lifecycle, backup, management-security, and UI state from those
   explicit paths and typed credentials.  The fixed backup/restore paths remain below the
   systemd StateDirectory; no HTTP input controls a filesystem path.
5. Add `deploy/systemd/cpa-rust-gateway.service` and an installation guide.  The unit uses
   `LoadCredential=`, `StateDirectory=`, `RuntimeDirectory=`, `LogsDirectory=`, private loopback
   binding, journald logging, unprivileged user/group, `UMask=0077`, bounded restart policy,
   explicit `LimitNOFILE`, `MemoryMax`, `CPUQuota`, `TasksMax`, and documented systemd sandbox
   controls.  It does not install, enable, restart, or otherwise change a host.
6. Add a deterministic unit verifier.  On Linux it invokes `systemd-analyze verify`; everywhere
   it checks the locked directives, rejects a unit that accepts ambient secret environment
   variables, and confirms the process command passes `%d` as the sole credential directory.

## Verification and review plan

- Focused Rust tests cover command parsing, listener/data-plane separation, secret-file rejection,
  typed management composition, and no start on an invalid configuration.
- The unit verifier covers systemd syntax (when available) and every locked deployment invariant.
  The real-Linux systemd `255` run caught and repaired an unsupported
  `ConditionPathIsExecutable=` directive; the corrected exact Unit SHA-256
  `f40cf0e55116360fe8372240131f4fa69aea5e6692f0ec64a21a9d859397063d` passed. P12-04 will repeat
  the check after creating only the verified binary path and before Unit installation.
- Run formatting, focused Clippy/tests, the Fast and Full local gates, docs/plan/secret scans, and
  an independent diff review.  Preserve the single active-task rule: P12-02 may become
  `LOCAL_PASS_PENDING_PHASE_GATE`; P12-03 must complete its prerequisite backup before P12-04
  becomes the only subsequent task. No staging installation can precede that backup.

## Exclusions and rollback

- No SSH/server login, unit installation, systemctl invocation against a host, database backup,
  provider/credential import, HTTP live probe, Caddy/Cloudflare/DNS change, registry push, tag,
  or GitHub Release.
- No public management exposure, trusted proxy headers, browser-origin widening, environment
  credential fallback, unbounded queue, or inference-runtime substitute.
- Rollback removes the `serve` composition, deployment assets, verifier, and documentation.  It
  does not modify a server or persisted database because this task performs neither action.
