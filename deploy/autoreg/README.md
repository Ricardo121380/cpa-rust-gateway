# Autoreg Oracle Singapore overlay

This overlay is the ARM64 deployment envelope for the migrated Autoreg project.
It is intentionally separate from CPAR and the retired Jakarta predecessor:

- project data lives under `/opt/example-autoreg/project`;
- the copied CPA auth pool lives under `/opt/example-autoreg/cliproxyapi-auths` and is mounted
  into the Autoreg container only;
- CloudMail environment files live under `/opt/example-autoreg/cloudmail`;
- WARP, Privoxy, and FlareSolverr listen only on loopback ports `21180`, `24080`, and
  `28191`; the console listens only on `127.0.0.1:18650`;
- `/opt/example-autoreg/project/deploy/autoreg/autoreg-oracle.service` is the optional durable
  systemd wrapper. It does not change Caddy, DNS, CPAR, the old CPA, or CC Switch.

The source Jakarta stack must remain running until an operator accepts the Oracle
instance and explicitly authorizes a cutover. Rollback is `docker compose down` for
the `autoreg-oracle` project plus restoration of the target preimage; no source data
is deleted by this overlay.
