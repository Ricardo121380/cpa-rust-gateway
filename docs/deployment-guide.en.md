# CPAR Deployment Guide

[简体中文](deployment-guide.zh-CN.md) | English

This guide describes how to build and operate CPAR without weakening its listener, credential or
Provider-isolation contracts. It is written for the current repository state as of 2026-08-20.

## 1. Support matrix

| Platform | Native source build | Container | Recommended role | Status |
|---|---:|---:|---|---|
| Linux x86-64, glibc | Yes | `linux/amd64` | Production/community self-hosting | Release workflow and systemd envelope exist |
| Linux AArch64, glibc | Yes | `linux/arm64` | Production/community self-hosting | Native ARM64 release runner exists |
| macOS Apple Silicon | Yes | Docker Desktop host mode is not a reviewed production path | Development and local evaluation | Local builds/gates are exercised |
| macOS Intel | Expected from the pinned toolchain | Not a reviewed production path | Development only | No current public release artifact |
| Windows | Use WSL2 | Docker/WSL2 development only | Development/evaluation | No native Windows service contract |
| Alpine/musl | Not currently supported | No Alpine runtime | None | Requires a separate dependency/toolchain review |
| ARMv7/32-bit | Not currently supported | No image | None | Requires a separate architecture and atomic-width review |
| Kubernetes | Binary can be containerized, but no reviewed manifests/operators exist | Not turnkey | Future | Listener and Secret patterns need a dedicated deployment contract |

The signed private release workflow recognizes only `x86_64-unknown-linux-gnu` and
`aarch64-unknown-linux-gnu`. Do not describe musl, ARMv7, Windows or Kubernetes as supported until
their build, runtime, secret, persistence and rollback paths have independent evidence.

## 2. Choose a deployment path

- Use **Docker/Compose** on a single Linux host when host networking and bind-mounted state are
  acceptable.
- Use **native systemd** for the strongest current host hardening and `LoadCredential=` handling.
- Use a **native foreground build** for macOS/Linux development and isolated evaluation.
- Approved maintainers may use the **signed private artifact workflow**; it is not a public
  download channel.

The root `Dockerfile` remains the signed-release runtime envelope: it expects a separately built
and verified file named `gateway`. Community source builds use
`deploy/docker/Dockerfile` instead.

At this documentation snapshot, the Compose file has passed static configuration validation, but a
Docker daemon was not available on the authoring Mac. The source-image build/run path is therefore
documented and bounded, not claimed as a completed local smoke test; run it on native Linux amd64
and arm64 builders before treating a community image as production-ready.

## 3. Shared security requirements

### 3.1 Listeners

CPAR accepts only loopback listeners and requires data and management to be different nonzero
addresses:

```text
gateway serve \
  --data-listen 127.0.0.1:18180 \
  --management-listen 127.0.0.1:18181 \
  --state-dir /var/lib/cpa-rust-gateway \
  --credential-dir /run/cpa-rust-gateway-credentials
```

- Reverse-proxy only `127.0.0.1:18180` after TLS termination.
- Never expose `127.0.0.1:18181` to a public load balancer, Caddy site, Docker `ports:` entry or
  firewall rule.
- For remote administration, use an authenticated SSH tunnel to the management listener.

### 3.2 Bootstrap credentials

Create six direct regular files. Symlinks, directories, special files, empty/oversized data and
incorrectly sized binary keys are rejected.

| Name | Requirement |
|---|---|
| `management-key` | ASCII value starting with `mgmt_`, total size 32–512 bytes |
| `management-csrf` | independent ASCII value starting with `csrf_`, total size 32–512 bytes |
| `master-key` | exactly 32 random raw bytes |
| `backup-key` | exactly 32 random raw bytes |
| `client-key-pepper` | exactly 32 random raw bytes |
| `grok-build-cache-key` | exactly 32 random raw bytes, used only for tenant-isolated Grok Build prompt-cache identities |

Example for a root-managed Linux host:

```bash
sudo install -d -o root -g root -m 0700 /etc/cpa-rust-gateway/credentials
sudo sh -c 'umask 077; printf "mgmt_%s" "$(openssl rand -hex 32)" > /etc/cpa-rust-gateway/credentials/management-key'
sudo sh -c 'umask 077; printf "csrf_%s" "$(openssl rand -hex 32)" > /etc/cpa-rust-gateway/credentials/management-csrf'
sudo sh -c 'umask 077; openssl rand -out /etc/cpa-rust-gateway/credentials/master-key 32'
sudo sh -c 'umask 077; openssl rand -out /etc/cpa-rust-gateway/credentials/backup-key 32'
sudo sh -c 'umask 077; openssl rand -out /etc/cpa-rust-gateway/credentials/client-key-pepper 32'
sudo sh -c 'umask 077; openssl rand -out /etc/cpa-rust-gateway/credentials/grok-build-cache-key 32'
```

Back up the key material separately from the SQLite state. A database backup without the matching
key ring cannot open encrypted credentials; a key backup without the database is not a runtime
backup. Never put values in Compose files, environment variables, shell arguments, images or Git.

### 3.3 State

The writable state directory contains at least:

- `control.sqlite3` — control plane, encrypted credentials and event/operations state;
- `backups/` — reviewed backup artifacts;
- `restore-target.sqlite3` when a restore is staged.

Use one process per state directory. Place it on a durable local filesystem and include the
database, WAL/SHM files when live, and external keys in an operator-controlled backup procedure.

## 4. Source build

### 4.1 Linux prerequisites

Debian/Ubuntu example:

```bash
sudo apt-get update
sudo apt-get install --yes \
  build-essential ca-certificates clang cmake curl git libclang-dev libssl-dev \
  nodejs npm pkg-config ripgrep
```

Install the pinned Rust toolchain and the Node version from `web/prism/.nvmrc`. A version manager
such as nvm, fnm or Volta is recommended when the distribution Node package is older.

On a new host, install `rustup` from its official bootstrap before installing the pinned toolchain:

```bash
command -v rustup >/dev/null || \
  curl --proto '=https' --tlsv1.2 --fail --silent --show-error \
    https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
source "$HOME/.cargo/env"
```

### 4.2 Compile a pinned revision

```bash
git clone https://github.com/Ricardo121380/cpa-rust-gateway.git
cd cpa-rust-gateway
git checkout <TAG_OR_COMMIT>

rustup toolchain install 1.97.1 --profile minimal --component rustfmt --component clippy
npm --prefix web/prism ci --ignore-scripts --no-audit --no-fund
npm --prefix web/prism run check

export GATEWAY_RELEASE_REVISION="$(git rev-parse HEAD)"
export GATEWAY_RELEASE_RUST_VERSION=1.97.1
export GATEWAY_RELEASE_TARGET="$(rustc +1.97.1 -vV | awk '/^host:/ {print $2}')"
CARGO_INCREMENTAL=0 cargo +1.97.1 build --locked --release --package gateway

./target/release/gateway --help
```

The Rust build invokes the Prism build and embeds exactly four reviewed assets. If
`web/prism/node_modules` is absent or the generated management client has drifted, Cargo correctly
fails rather than shipping a stale UI.

### 4.3 Local smoke test

The repository includes a synthetic, loopback-only smoke test:

```bash
./scripts/test-p12-02-serve.sh
```

It creates temporary synthetic keys, starts both listeners, checks `/healthz`, management
separation and the embedded UI, then destroys the temporary state. It does not contact a Provider.

## 5. Docker source build

### 5.1 Build the host architecture

```bash
docker buildx build \
  --file deploy/docker/Dockerfile \
  --build-arg CPAR_REVISION="$(git rev-parse HEAD)" \
  --build-arg CPAR_VERSION=source \
  --load \
  --tag cpar:local \
  .
```

This is a community/source image. It does not inherit the Sigstore signature, SBOM or immutable
receipt of the private release workflow.

### 5.2 Multi-architecture build

With a BuildKit builder that can execute both platforms:

```bash
docker buildx build \
  --file deploy/docker/Dockerfile \
  --platform linux/amd64,linux/arm64 \
  --build-arg CPAR_REVISION="$(git rev-parse HEAD)" \
  --build-arg CPAR_VERSION=source \
  --output type=oci,dest=cpar-source.oci.tar \
  .
```

Native builders are preferred. QEMU/emulation may build a community image but is not equivalent to
the project's signed release evidence, which builds and smoke-tests each target on a native runner.

### 5.3 Prepare host directories

The runtime image uses numeric UID/GID `65532`. The state mount must be writable by that identity;
the credential mount must be readable/traversable but not writable.

```bash
sudo install -d -o 65532 -g 65532 -m 0700 /srv/cpar/state
sudo install -d -o 65532 -g 65532 -m 0500 /srv/cpar/credentials
# Create the six files securely, then:
sudo chown 65532:65532 /srv/cpar/credentials/*
sudo chmod 0400 /srv/cpar/credentials/*
```

### 5.4 Run on Linux

```bash
docker run -d --name cpar \
  --restart unless-stopped \
  --network host \
  --read-only \
  --user 65532:65532 \
  --cap-drop ALL \
  --security-opt no-new-privileges:true \
  --memory 768m --cpus 2 --pids-limit 512 \
  --ulimit nofile=65536:65536 \
  --tmpfs /tmp:rw,noexec,nosuid,nodev,size=16m \
  --mount type=bind,src=/srv/cpar/state,dst=/var/lib/cpa-rust-gateway \
  --mount type=bind,src=/srv/cpar/credentials,dst=/run/cpar-credentials,readonly \
  cpar:local serve \
  --data-listen 127.0.0.1:18180 \
  --management-listen 127.0.0.1:18181 \
  --state-dir /var/lib/cpa-rust-gateway \
  --credential-dir /run/cpar-credentials

curl --noproxy '*' --fail http://127.0.0.1:18180/healthz
```

Linux host networking is required because CPAR deliberately binds only container loopback. Bridge
mode `-p 18180:18180` cannot reach a process listening on `127.0.0.1` inside the container. Docker
Desktop host networking is not a reviewed production path.

## 6. Docker Compose

```bash
export CPAR_IMAGE=cpar:local
export CPAR_STATE_DIR=/srv/cpar/state
export CPAR_CREDENTIAL_DIR=/srv/cpar/credentials
export CPAR_REVISION="$(git rev-parse HEAD)"
export CPAR_VERSION=source

docker compose -f deploy/docker/compose.yml config
docker compose -f deploy/docker/compose.yml up -d --build
curl --noproxy '*' --fail http://127.0.0.1:18180/healthz
```

The Compose file intentionally has no `ports:` section and is Linux-only due to `network_mode:
host`. It uses a read-only root filesystem, drops capabilities, sets resource limits and mounts
state/credentials separately. `docker compose down -v` does not back up or delete these bind
mounts; explicit deletion or overwrite of `CPAR_STATE_DIR` is destructive.

## 7. Native Linux + systemd

### 7.1 Install the binary and account

```bash
sudo groupadd --system cpa-gateway 2>/dev/null || true
sudo useradd --system --gid cpa-gateway --home-dir /var/lib/cpa-rust-gateway \
  --shell /usr/sbin/nologin cpa-gateway 2>/dev/null || true

revision="$(git rev-parse HEAD)"
sudo install -d -o root -g root -m 0755 "/opt/cpa-rust-gateway/releases/$revision"
sudo install -o root -g root -m 0755 target/release/gateway \
  "/opt/cpa-rust-gateway/releases/$revision/gateway"
sudo ln -sfn "/opt/cpa-rust-gateway/releases/$revision" /opt/cpa-rust-gateway/current
```

Create the root-owned credentials as described above, then install and verify the reviewed unit:

```bash
ruby scripts/check-p12-02-systemd-unit.rb
sudo install -o root -g root -m 0644 deploy/systemd/cpa-rust-gateway.service \
  /etc/systemd/system/cpa-rust-gateway.service
sudo systemd-analyze verify /etc/systemd/system/cpa-rust-gateway.service
sudo systemctl daemon-reload
sudo systemctl enable --now cpa-rust-gateway
sudo systemctl status cpa-rust-gateway --no-pager
curl --noproxy '*' --fail http://127.0.0.1:18180/healthz
```

The unit creates the state/runtime/log directories, hands credentials through systemd's read-only
credential directory, runs without capabilities and applies filesystem/kernel/syscall hardening.

## 8. First boot and configuration

`/healthz` can be healthy with an empty database. Inference remains unavailable until an operator:

1. opens the loopback Prism UI or management API;
2. creates a draft Config Version;
3. configures egress policy, Upstream, Endpoint, Credential and exact binding;
4. creates public models, routes/candidates, access groups and Client Keys;
5. validates and publishes the Config Version;
6. restarts CPAR so the active immutable runtime graph is composed;
7. verifies `/v1/models`, then one bounded request for the intended Provider/Channel.

For remote administration, tunnel rather than publish management:

```bash
ssh -L 18181:127.0.0.1:18181 operator@your-server
```

Open `http://127.0.0.1:18181/admin-ui/` locally. The detailed resource order, opaque-ID ledger,
canary and rollback rules are in `docs/p12-rollout-runbook.md`.

## 9. Reverse proxy and TLS

Use Caddy, Nginx or another reviewed reverse proxy to terminate TLS and proxy only the data plane.
The repository's `deploy/caddy` files are templates with placeholder hosts, not drop-in production
configuration.

Important boundaries:

- do not proxy management port `18181`;
- avoid response buffering/compression that changes SSE framing;
- do not apply global Caddy `servers` timeouts to unrelated sites;
- preserve WebSocket upgrade headers;
- set upstream connect/header/idle timeouts deliberately;
- validate `/healthz`, JSON, SSE and WebSocket separately before traffic cutover.

## 10. Upgrade, backup and rollback

1. Record the current binary SHA, Config Version, database schema and key versions.
2. Create a filesystem/SQLite-consistent backup and hash receipt; back up key material separately.
3. Build and verify the successor revision without replacing `current`.
4. Stop CPAR, preserve the old database, switch the `current` symlink (or image digest), and start.
5. Check `/healthz`, management preflight, `/v1/models` and a bounded canary.
6. If acceptance fails, stop the successor, restore the prior binary/image and compatible database,
   then verify the rollback receipt.

Do not downgrade across irreversible schema/key changes without the matching migration/restore
contract. Management restore stages into `restore-target.sqlite3`; it does not silently overwrite
the live database.

## 11. Signed private release artifacts

`.github/workflows/release-artifact.yml` is manual and currently produces private 14-day workflow
artifacts for Linux amd64/arm64. Each bundle contains a binary, OCI archive, CycloneDX SBOM,
manifest, Sigstore bundle and receipt. It does not publish GHCR or a GitHub Release.

Verify an extracted artifact bundle against its exact expected identity before importing it:

```bash
ruby scripts/p12-release-artifact.rb verify \
  --artifact-dir <EXTRACTED_ARTIFACT_DIR> \
  --revision <EXACT_40_CHARACTER_SHA> \
  --rust-toolchain 1.97.1 \
  --target <x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu> \
  --version 0.1.0 \
  --require-signature \
  --require-receipt

ruby scripts/p12-release-artifact.rb inspect-oci \
  --artifact-dir <EXTRACTED_ARTIFACT_DIR> \
  --revision <EXACT_40_CHARACTER_SHA> \
  --rust-toolchain 1.97.1 \
  --target <x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu> \
  --version 0.1.0
```

The Ruby verifier checks the closed file set, digests, build metadata, architecture, OCI layout,
SBOM and receipt. `--require-signature` requires the Sigstore files and their receipt coverage, but
it does **not** perform the cryptographic signature verification itself. Install Cosign and verify
the manifest separately against an identity fixed outside the downloaded artifact:

```bash
cosign verify-blob \
  --bundle <EXTRACTED_ARTIFACT_DIR>/artifact-manifest.sigstore.json \
  --certificate-identity \
    'https://github.com/Ricardo121380/cpa-rust-gateway/.github/workflows/release-artifact.yml@<EXPECTED_REF>' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  <EXTRACTED_ARTIFACT_DIR>/artifact-manifest.json
```

Replace `<EXPECTED_REF>` with the independently trusted release ref. Do not accept an arbitrary
certificate identity merely because it appears in the downloaded `signing-identity.json`; that
would let an attacker choose both the artifact and its alleged trust root.

The formal `gateway-image.oci.tar` is an OCI archive, not a Docker archive. Import it with an OCI
tool, for example:

```bash
skopeo copy \
  oci-archive:gateway-image.oci.tar \
  docker-daemon:cpa-rust-gateway:<REVISION>
```

`docker load` is appropriate only for a Docker archive, not the formal OCI layout. Complete both
the structural/digest verification and the independent Cosign verification before use.

## 12. Automatic OAuth renewal and rollback

When the active Config Version contains a refreshable OAuth channel supported by CPAR,
`gateway serve` runs one bounded catch-up before compiling serving pools and starts a one-minute
background check after both listeners bind. The current production composition covers native Grok
Build OAuth and exact official Codex OAuth. Static API keys and Grok Console/Web SSO cookies are not
misclassified as refreshable OAuth.

Operators should inspect only redacted counters and states: `claimed`, `succeeded`, `backed_off`,
`reauth_required` and `runtime_replaced`. Logs must not contain access/refresh tokens, account
identity or Provider response bodies. Failed Codex refreshes must not contact the upstream every
minute without a bound: the current worker uses process-local exponential `1/2/4/.../60` minute
backoff and clears it immediately after success. `reauth_required` means the stored refresh grant
can no longer recover automatically; stop leasing that Credential and let the operator/Autoreg
perform interactive authorization. Routine renewal does not depend on the Autoreg service being
online.

Keep both the old binary symlink and a SQLite preimage verified with `quick_check` and foreign-key
checks before an upgrade. Startup catch-up may rotate encrypted Credential material before listener
bind. If startup then fails after a rotation, restore the database preimage together with the old
binary; switching only the executable is not a complete rollback. A request that already owns a
lease finishes on its pinned old secret revision, while later leases observe the atomic replacement.

Before activating another refreshable Provider, implement its exact Provider/channel executor,
egress, CAS/backoff and restart tests. Never select a Codex or Grok refresh protocol merely from an
`oauth_json` label or a compatible-looking JSON document.

## 13. Troubleshooting

| Symptom | Check |
|---|---|
| Cargo says Prism dependencies are missing | Run the exact `npm --prefix web/prism ci ...` command. |
| Container starts but host cannot connect | Confirm Linux `--network host`; bridge `ports:` is incompatible with loopback-only bind. |
| Credential rejected | Verify exact filename, direct regular file, size/prefix, permissions and no symlink. |
| `/healthz` works but inference does not | Publish a complete Config Version, issue a Client Key and restart. |
| Management returns not found | Confirm Management Key, loopback peer/origin and exact Config Version headers. |
| SSE appears buffered | Disable reverse-proxy buffering/compression and inspect idle/read timeouts. |
| Old cursor/revision conflicts | Restart the bounded read from page one or re-read the current ETag/revision. |
| Database cannot decrypt after restore | Restore the matching external master/backup key material; do not guess or replace keys. |

## 14. Deliberate non-goals

This guide does not authorize real Provider probes, account registration, Autoreg, public
management exposure, interactive credential reauthorization/account repair, production Grok Web
clearance, Kubernetes or unsupported architectures. Routine automatic renewal of an imported,
supported OAuth grant is normal CPAR runtime behavior; it does not authorize registration or repair
of a revoked refresh grant.
