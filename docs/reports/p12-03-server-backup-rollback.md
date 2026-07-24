# P12-03 server backup and rollback receipt

| Field | Value |
|---|---|
| Plan version | `v1.47` |
| Task | `P12-03` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Backup timestamp | `2026-07-24T19:02:26Z` |
| Scope | One server-local, root-private snapshot of the existing CPA data root and associated manager-unit fragment. No new Unit, listener, staging instance, proxy, DNS, Cloudflare, credential, database row, request, or traffic changed. |

## Incumbent boundary

The precise incumbent data-plane boundary is Docker container `cli-proxy-api`, running image
`eceasy/cli-proxy-api:v7.2.98` with immutable local image identity
`sha256:8169bad5fafc95f2d8e798494343b796661358440df9cd3479bdb41f8cecb5f6`.
It was running with Docker restart policy `no` at snapshot time and has exactly the reviewed
state mount `/opt/example-legacy-gateway/cpa` to container `/data`.  The source root was `0755 root:root`.

`cpa-manager-plus.service` was active, but its fragment is retained only as related operational
metadata: this receipt does not claim that it is the direct supervisor of `cli-proxy-api`.
Its static fragment was copied separately from
`/etc/systemd/system/cpa-manager-plus.service` (`0644 root:root`, SHA-256
`a0bae4e0576e992dd0f6d26f14f21ae706134cbe57ac92a9d45e4116236b9e62`).

No SQLite-family file was present in the data root, so a SQLite online backup was not applicable.
The file-store snapshot contains 834 regular files and 8 directories after excluding the top-level
`logs` tree.  No file names, configuration values, credentials, environment variables, database
rows, or request logs were collected in this repository or this receipt.

## Server-local backup set

The backup resides only in the server-local, root-owned directory
`/var/backups/cpa-rust-gateway/p12-03-20260724T190226Z` (`0700 root:root`).  Every artifact in that
directory is mode `0600`; no archive was copied to the workstation, Git, or chat.

| Artifact | Coverage | Bytes | SHA-256 |
|---|---|---:|---|
| `cpa-state.tar` | `/opt/example-legacy-gateway/cpa`, preserving metadata and excluding `cpa/logs` | 26,091,520 | `2025e9dbe3a30a9248ede72e81c02dda89e72fa3a43af1102576b8b5198b2aab` |
| `cpa-manager-plus-unit.tar` | One systemd unit fragment, retained as operational context | 10,240 | `49d8bbebfc65d2b5153c3f786c8ece657c5707bd3612f39865540644ac5552d0` |
| `manifest.txt` | Value-free scope, identities, ownership, hashes and rollback commands | — | `d909525e290fa8a806f7f0305f34d48b94868ac2a926153612a76a2f705ab92c` |

## Consistency and integrity review

Before archiving, the ext4 data root had no writable open file descriptor.  A content aggregate
over all non-log regular files was calculated before and after the archive; both were
`d67d25836aeddd83cfa6a73f77ef667457168e638f40aed3fbcba9e44a78aa45`.
The source was therefore quiescent for the complete archive interval without stopping or pausing
the incumbent.

An independent post-write review verified all required manifest fields, both archive hashes,
archive readability, artifact modes, and the absence of `cpa/logs` entries.  All checks passed.
The earlier read-only discovery deviation that listed credential-store file names was not retained
here; it read no content or values, and subsequent discovery used aggregate counts only.

## Local documentation gate

`CHECK_REPORT_PATH=docs/reports/evidence/p12-03-docs-gate-20260725.md ./scripts/check.sh docs`
passed in seven seconds.  It verified document links, plan-state consistency, the all-tracked
Secret scan, and Git whitespace.  The retained value-free receipt is
[p12-03-docs-gate-20260725.md](evidence/p12-03-docs-gate-20260725.md).

## Exact rollback procedure

These commands are stored in the server-local manifest and were **not executed** during P12-03.
They revert only the incumbent CPA state and the retained unit fragment; they do not install or
start the new gateway Unit.

```bash
sudo docker stop cli-proxy-api
sudo find /opt/example-legacy-gateway/cpa -mindepth 1 -maxdepth 1 ! -name logs -exec rm -rf -- {} +
sudo tar --acls --xattrs --numeric-owner -xf \
  /var/backups/cpa-rust-gateway/p12-03-20260724T190226Z/cpa-state.tar \
  -C /opt/example-legacy-gateway
# Only if the retained manager-unit fragment also needs restoration:
sudo tar --acls --xattrs --numeric-owner -xf \
  /var/backups/cpa-rust-gateway/p12-03-20260724T190226Z/cpa-manager-plus-unit.tar \
  -C /
sudo systemctl daemon-reload
sudo docker start cli-proxy-api
sudo docker inspect --format '{{.State.Running}}' cli-proxy-api
```

The non-log root is cleared before extraction so files added after the snapshot cannot survive the
restore.  The existing logs directory is deliberately preserved; it is outside the configuration,
authentication, plugin, and state recovery set.  This P12-03 receipt unlocks only the local
P12-04 prerequisite.  P12 remains pending its integrated local gate and its single Delivery Gate;
no production change has been accepted.
