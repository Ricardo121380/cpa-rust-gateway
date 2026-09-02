# Claude Code handoff: Oracle Singapore VPS access

Last verified: 2026-08-26 (Asia/Shanghai)

This document is a value-free connection handoff for Claude Code. It intentionally contains no
public IP address, private-key path, password, Management Key, CSRF token, Provider credential,
cookie, database row, request/response body, or raw remote log.

## 1. Current verified topology

The answer to “is CPAR configured on the Oracle Singapore VPS?” is **yes, with an important
qualification**:

| SSH alias | Verified environment | CPAR | Autoreg | Known loopback listeners |
|---|---|---|---|---|
| `new-vps` | Oracle Cloud `ap-singapore-1`, Ubuntu 24.04, `aarch64` | `cpa-rust-gateway.service` is `active/running` | `autoreg-oracle.service` is `active/exited`; this is the Compose wrapper's expected systemd shape | CPAR data `18180`, CPAR management `18181`, Autoreg console `18650` |
| `jakarta-vps` | Ubuntu 24.04, `x86_64`; Oracle metadata not detected | `cpa-rust-gateway.service` is still `active/running` | Oracle Autoreg unit and `18650` listener are absent | CPAR data `18180`, CPAR management `18181` |

On `new-vps`, Caddy is also active, `/opt/cpa-rust-gateway/current` exists, CPAR state is under
`/var/lib/cpa-rust-gateway`, and the CPAR unit runs as `cpa-gateway:cpa-gateway` with that state
directory as its working directory.

The 2026-08-10 receipts record Oracle CPAR public canaries and the Autoreg single-active cutover.
However, the 2026-08-26 read-only check also found the old Jakarta **CPAR** process still active.
Do not infer current DNS/Caddy traffic ownership from process state alone, and do not stop either
host as part of frontend work.

### CPAR and Autoreg are separate projects

- CPAR is the gateway, management UI, provider runtime, credential pool, health/quota state, and
  public data plane.
- Autoreg is an external account-source lifecycle project. It may produce a credential package for
  explicit import, but it is not a CPAR Provider, route, upstream, database, or scheduler.
- CPAR itself performs routine renewal for an already-imported, active and supported refreshable
  OAuth grant. Autoreg remains responsible for registration, initial authorization, replenishment
  and interactive reauthorization after a refresh grant is revoked. Do not diagnose normal token
  rotation by reading or restarting Autoreg.
- Sharing one Oracle VM does not merge their ownership or authorization boundaries.
- The committed Autoreg overlay uses example paths. The live Oracle unit currently reports
  `WorkingDirectory=/opt/autoreg/project`; never assume a template path when inspecting the live
  host. Query the unit metadata first, without printing its environment or configuration content.

Relevant durable evidence:

- `docs/change-requests/CR-P12-AUTOREG-SEPARATION-001.md`
- `docs/change-requests/CR-P13-RUNTIME-OAUTH-REFRESH-001.md`
- `docs/reports/evidence/autoreg-oracle-single-active-cutover-20260810.md`
- `docs/reports/evidence/autoreg-oracle-cpar-production-canary-20260810.md`
- `docs/reports/evidence/autoreg-oracle-console-production-canary-20260810.md`

## 2. Local SSH entry point

The operator Mac already has an SSH config entry named `new-vps`. Use the alias; do not replace it
with an IP address and do not copy the expanded SSH configuration into chat, commits, screenshots,
or test output.

Safe handshake:

```bash
ssh -o BatchMode=yes \
  -o ConnectTimeout=10 \
  -o ServerAliveInterval=5 \
  -o ServerAliveCountMax=1 \
  new-vps 'printf "oracle_ssh=ok\n"'
```

Rules:

1. Keep normal host-key verification enabled. Never use `StrictHostKeyChecking=no`, delete the
   known-host entry, or accept a changed fingerprint without independent operator verification.
2. Keep `BatchMode=yes` for automation. If key authentication fails, stop; do not fall back to a
   password prompt or copy a private key.
3. Do not read, print, copy, upload, or commit `~/.ssh/config`, `ssh -G new-vps` output, private-key
   material, or the resolved hostname/IP.
4. The alias currently supplies the remote user, port, hostname, and identity. Claude Code should
   not duplicate those values in repository files.

## 3. Approved read-only preflight

Run this before any real management-UI verification:

```bash
ssh -o BatchMode=yes -o ConnectTimeout=10 new-vps '
  set -u
  printf "cpar=%s\n" "$(systemctl is-active cpa-rust-gateway.service 2>/dev/null || true)"
  printf "autoreg=%s\n" "$(systemctl is-active autoreg-oracle.service 2>/dev/null || true)"
  printf "caddy=%s\n" "$(systemctl is-active caddy.service 2>/dev/null || true)"
  for port in 18180 18181 18650; do
    if [ -n "$(ss -H -ltn "sport = :${port}" 2>/dev/null)" ]; then
      printf "listener_%s=present\n" "$port"
    else
      printf "listener_%s=absent\n" "$port"
    fi
  done
'
```

An `active (exited)` Autoreg wrapper is not by itself a failure: the systemd unit launches a
detached Compose stack. The loopback listener and a separately authorized health check are the
relevant follow-up observations.

Safe CPAR health check, without exposing the data listener publicly:

```bash
ssh -o BatchMode=yes new-vps \
  'curl --noproxy "*" --fail --silent --show-error \
    http://127.0.0.1:18180/healthz >/dev/null && printf "cpar_health=ok\n"'
```

This proves only process/listener health. It does not prove Provider credentials, public DNS,
Caddy routing, model availability, quota, or successful upstream inference.

## 4. CPAR management UI tunnel

The management listener is loopback-only by design. Do not expose port `18181` through Caddy,
a cloud firewall, Docker port publishing, or a public bind. Use an SSH local forward.

First ensure local port `18181` is free:

```bash
lsof -nP -iTCP:18181 -sTCP:LISTEN
```

If another process owns the port, identify it and stop for operator direction; do not kill it
automatically. Then start the tunnel in a dedicated terminal:

```bash
ssh -N -T \
  -o BatchMode=yes \
  -o ExitOnForwardFailure=yes \
  -o ServerAliveInterval=30 \
  -o ServerAliveCountMax=3 \
  -L 18181:127.0.0.1:18181 \
  new-vps
```

Open:

```text
http://127.0.0.1:18181/admin-ui/
```

The local port must remain `18181` for full mutation testing. CPAR derives the allowed browser
origin from its management listener; forwarding to an arbitrary local port can make GETs work while
unsafe operations fail closed with `404 management_access_denied`.

Management Key and CSRF material are operator secrets. They must be supplied out of band and entered
manually or through an explicitly approved secret channel. Do not retrieve them with `sudo cat`,
place them in command history, echo them, paste them into chat, save them in browser storage, or add
them to fixtures. Close the tunnel with `Ctrl-C` when verification ends.

## 5. Autoreg console tunnel is a different session

Only use this when the operator explicitly asks for Autoreg work. It is not part of routine CPAR
frontend verification:

```bash
ssh -N -T \
  -o BatchMode=yes \
  -o ExitOnForwardFailure=yes \
  -o ServerAliveInterval=30 \
  -o ServerAliveCountMax=3 \
  -L 18650:127.0.0.1:18650 \
  new-vps
```

Then access the operator-approved Autoreg console origin on local port `18650`. Autoreg credentials,
account records, browser state, CloudMail data, cookies, and registration logs are outside CPAR and
must not be copied into this repository or used as frontend fixtures.

## 6. Frontend deployment boundary

Prism is compiled and embedded into the `gateway` binary. There is no supported operation that
copies `web/prism/dist` directly onto Oracle. Therefore:

1. Complete and review frontend changes locally on a clean commit.
2. Run the repository's contract synchronization and frontend checks.
3. Build the complete gateway artifact through the reviewed release path.
4. Verify artifact identity, checksums, SBOM, and signing evidence.
5. Obtain a separate deployment authorization with backup, rollback, health, and traffic scope.
6. Only then may an operator install the artifact and restart the CPAR unit.

This handoff authorizes **connection and read-only inspection only**. It does not authorize upload,
deployment, `git pull`, service restart, Docker/Compose mutation, SQLite access, Provider requests,
Caddy/DNS/firewall changes, Autoreg scheduling, credential import, traffic cutover, or rollback.

## 7. Commands and outputs that are forbidden by default

Do not run or paste the output of:

- `env`, `printenv`, `systemctl show ... -p Environment`, or unredacted `systemctl cat`;
- `docker inspect`, unbounded `docker logs`, or unbounded `journalctl`;
- `cat`/`sed`/`rg` over credential, environment, configuration, backup, database, or auth-pool files;
- `sqlite3` queries against production state;
- `systemctl restart|stop|disable`, `docker compose up|down`, `docker rm`, `git pull/reset`, `rm`,
  Caddy reloads, firewall rules, or DNS changes;
- real Channel Pin, Provider probe, account action, registration, refresh, or inference calls unless
  the operator creates a separate explicit execution boundary.

If a diagnostic needs any of those surfaces, stop and request a narrowly scoped approval. Return
only value-free summaries: service state, fixed error class, count, timestamp, revision, checksum,
or PASS/FAIL. Never return raw request/response bodies or account identities.

## 8. Failure handling

| Symptom | Required response |
|---|---|
| Host-key mismatch | Stop immediately and ask the operator to verify the new fingerprint out of band. |
| Batch authentication failure | Stop; do not request or copy a password/private key. |
| Local port `18181` already in use | Identify the owner value-free and request direction; do not silently choose another port for mutation testing. |
| CPAR inactive or listener absent | Report the state only; do not restart it. |
| Oracle and Jakarta observations disagree | Treat topology as ambiguous; do not stop, deploy, or cut traffic on either host. |
| UI GET works but mutation returns `404 management_access_denied` | Verify the exact `18181` tunnel/origin before changing frontend or backend code. |
| A command could reveal a secret or Provider payload | Do not run it; replace it with a bounded count/status/hash check or request approval. |

## 9. Completion receipt for a read-only Claude Code session

At the end, report only:

```text
ORACLE_SSH_HANDSHAKE=PASS|FAIL
CPAR_SERVICE_STATE=<closed state>
CPAR_HEALTH=PASS|FAIL|NOT_RUN
MANAGEMENT_TUNNEL=PASS|FAIL|NOT_RUN
PRISM_REAL_UI_CHECK=PASS|FAIL|NOT_RUN
REMOTE_MUTATION_COUNT=0
PROVIDER_REQUEST_COUNT=0
SECRET_OUTPUT_COUNT=0
```

Do not claim deployment, production traffic, Provider health, or Autoreg health from an SSH
handshake or management-page render alone.
