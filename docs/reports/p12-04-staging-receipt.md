# P12-04 isolated Staging receipt

| Field | Value |
|---|---|
| Plan version | `v1.48` |
| Task | `P12-04` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Artifact revision | `111f60a416fd0a6b4a6314bac8ff32b0074cdca7` |
| Artifact provenance | Private GitHub Actions run `30142690538`, artifact ID `8615022248`; independently verified GitHub OIDC keyless signature and Rekor record before any server write. |
| Binary SHA-256 | `305f7ddaa1ea0d7fd401a5ccc98b1eae6dd9f9798801b1ac8fad9cb1dedce3c3` |
| Unit SHA-256 | `f40cf0e55116360fe8372240131f4fa69aea5e6692f0ec64a21a9d859397063d` |
| Scope | One new, disabled-at-boot, loopback-only Staging Unit. No Provider, Client Key, Route, Upstream, inference request, Caddy, Cloudflare, DNS, public listener, incumbent credential, or incumbent configuration changed. |

## Provenance and installation checks

The private artifact was copied into a new disposable local directory containing only its allowed
nine files before the repository verifier, OCI inspector, and Cosign `verify-blob` keyless check
were repeated.  The verified identity was the repository's pinned `release-artifact.yml` workflow
on `refs/heads/codex/p12-deployment`; the issuer was GitHub Actions OIDC.  The Linux `x86_64`
server accepted the binary only after its SHA-256 matched the manifest record above.

Before the Unit was installed, the staged Linux binary passed `gateway serve --help`, and systemd
`255` accepted the exact Unit bytes under `systemd-analyze verify`.  The transient validator file
was named `cpa-rust-gateway.service` inside a disposable directory because systemd rejects a
temporary filename ending in `.tmp`; no temporary Unit was installed.  Existing server discovery
confirmed the two loopback ports, new paths, service account, and Unit name were all absent, and
about 10.5 GiB remained free on the root filesystem.

The installed binary is root-owned mode `0755` below its new release path.  The new Unit is
root-owned mode `0644`, has the SHA-256 above, and is **active but disabled**: it was deliberately
not enabled at boot.  This leaves an explicit operator choice before any persistence change.

## Runtime acceptance

| Check | Result |
|---|---|
| Process identity | PASS — the active main process runs as `cpa-gateway:cpa-gateway`. |
| Listener isolation | PASS — exactly `127.0.0.1:18180` and `127.0.0.1:18181` listen; no `0.0.0.0` or IPv6-any listener exists for either port. |
| Readiness surface | PASS — `HEAD /` returns `200`; `GET /healthz` returns the fixed readiness body. The data listener returns `404` for the management path. |
| Management admission | PASS — `POST /admin/backups/preflight` without a Management Key returns the existing P10 hidden denial `404`; the same request with the root-only Key supplied through curl stdin returns the expected safe preflight response. No credential value entered a command argument, output, receipt, Git, or chat. |
| Credential boundary | PASS — exactly five direct regular credential files are `root:root 0600`; two text credentials are 53 bytes and the three raw keys are exactly 32 bytes. |
| State boundary | PASS — systemd-created state, runtime, and log directories are `cpa-gateway:cpa-gateway 0700`. |
| Resource boundary | PASS — live Unit values are `MemoryMax=768M`, `CPUQuota=200%`, and `TasksMax=512`. |
| Journal and incumbent | PASS — the new Unit's value-free journal summary contains no startup/credential/panic failure; `cli-proxy-api` remained running throughout. |

The hidden `404` is intentional, not an authorization bypass: the P10 security suite asserts
`NOT_FOUND` for requests lacking the Management Key, including this exact backup-preflight route.

## Execution deviations and recovery

Three fail-closed operational observations were resolved without changing the signed binary,
server incumbent, or public surface:

1. `systemd-analyze verify` rejects a temporary filename ending in `.tmp`; validation was retried
   with the exact Unit filename in a disposable directory and passed.
2. The first credential-generation helper combined `pipefail` with a `tr | head` truncation. The
   upstream `tr` correctly received SIGPIPE, so the setup transaction exited before Unit
   installation or start and removed all new account/path/credential state. Read-only follow-up
   confirmed all P12 paths absent and the incumbent still running. The accepted helper instead
   converts exactly 24 random bytes into 48 hexadecimal characters without truncating a pipeline.
3. The initial receipt harness expected `403` for an unauthenticated management request. Source
   tests establish the stronger endpoint-hiding `404` contract, so the harness was corrected to
   assert `404` and separately prove the authenticated `200` preflight.

These are deployment-procedure corrections only. The committed P12-02 Unit repair remains the
sole source change; all artifact-producing inputs remain unchanged from the signed revision.

## Independent review and exact rollback

The review checked artifact and Unit digests, the server's actual listener table, root-only
credential metadata without reading their values, service/process ownership, request outcomes,
resource settings, journal summary, and incumbent continuity.  It found no P12-04 blocker.

The following commands revert only this new Staging instance; they do not touch the incumbent
container, `/opt/example-legacy-gateway/cpa`, Caddy, Cloudflare, DNS, or any Provider configuration:

```bash
sudo systemctl stop cpa-rust-gateway.service
sudo rm -f /etc/systemd/system/cpa-rust-gateway.service
sudo systemctl daemon-reload
sudo rm -rf /etc/cpa-rust-gateway /var/lib/cpa-rust-gateway \\
  /run/cpa-rust-gateway /var/log/cpa-rust-gateway /opt/cpa-rust-gateway
sudo userdel cpa-gateway
```

P12-04 does not authorize P12-05's test Upstream/Key write or any real Provider request. Those
must remain an explicit, separately reviewed next task.
