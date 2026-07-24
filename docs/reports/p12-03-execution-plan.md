# P12-03 server backup and rollback inventory plan

| Field | Value |
|---|---|
| Plan version | `v1.47` |
| Task | `P12-03` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Scope | Current-server inventory plus one timestamped, server-local backup set for the existing gateway configuration, state, version identity, and rollback instructions. |
| Preconditions | P12-01 accepted artifact; P12-02 locally accepted deployment envelope. P12-04 staging is outside this task and had not started. |

## Safety boundary

The first remote action is read-only discovery through the already configured SSH host alias.  It
may list service/container names, executable versions, candidate paths, owners, modes, sizes,
mounts, and database types, but it must not print configuration content, environment variables,
credentials, request logs, Provider data, or database rows.

Only after the live paths and service owner are identified will the task create a timestamped,
server-local backup directory with restrictive permissions.  It will copy only the current
gateway's configuration and state, use SQLite's consistent `.backup` mechanism where applicable,
record a value-free manifest (paths, ownership/mode, sizes, hashes, version identity and restore
commands), and verify archive/database readability.  No backup material may be transferred to this
repository, Git, an external host, or a chat response.

For a plain file-store with no database, the equivalent snapshot procedure is fail-closed: no
writable descriptor may be open in the source root, an aggregate of all included regular files
must match immediately before and after archiving, and any mismatch removes the partial backup and
stops the task.  It never pauses or restarts the incumbent to obtain that evidence.

## Execution sequence

1. Record operating-system, storage, service-manager and container-runtime availability, then
   identify only existing CPA/AxonHub/New API/Kiro-RS-related services, containers, working
   directories and candidate configuration/state paths.
2. Determine the actual incumbent gateway, its executable/image version, configuration root,
   persistent database/volume location, process owner, and normal start/reload/rollback command.
   Stop if the live topology is ambiguous or a required source is inaccessible.
3. Create a new timestamped private backup root on the same server.  Preserve file metadata and
   gather a redacted manifest; use SQLite online backup for a live SQLite database, or the
   fail-closed quiescent file-store procedure above when the actual state is plain files.
4. Check archive/backup integrity without outputting contents; verify that manifest hashes match
   and that rollback instructions restore only the incumbent service.  Do not execute rollback,
   restart, deployment, Caddy/Cloudflare, DNS, provider, or traffic changes.
5. Commit only the value-free local receipt, execution plan and plan status after independent
   review and local documentation/Secret checks.

## Acceptance criteria and stop conditions

- The receipt identifies one precise incumbent service boundary, timestamped backup root,
  config/state/version coverage, permissions, integrity results, and an unambiguous rollback
  procedure without Secret values.
- Any unknown active gateway, missing persistent data path, a non-SQLite database without a safe
  native snapshot, a file-store with writable descriptors or non-matching before/after aggregate,
  insufficient space, permission failure, or unexpected service topology is a hard stop; record
  the value-free condition and request direction before changing the server.
- P12-04 cannot install or start the new Unit until this receipt is accepted.  P12-03 does not
  create a staging service, network listener, proxy rule, DNS entry, or production traffic change.
