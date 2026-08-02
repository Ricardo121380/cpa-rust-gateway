# P12-08F3 client migration dry-run

Status: `LOCAL_PASS_PENDING_PHASE_GATE`

Date: 2026-08-02

This report is value-free. It records no endpoint, client key, model name, request or response body,
credential, source hash, or token fingerprint.

## Operator boundary

The operator allowed other clients to be tested but explicitly excluded CC Switch from this
rehearsal. Accordingly, this task did not read, copy, modify, restart, or test CC Switch, its
database, its generated settings, or any process inheriting its environment. CC Switch is
`DEFERRED_BY_OPERATOR`; the existing inventory entry remains historical evidence only.

OpenClaw is the only currently discovered standalone client inside the approved boundary. Codex
through CC Switch is covered by the same exclusion, and no additional standalone CPA client was
discovered on this Mac mini.

## Isolated OpenClaw rehearsal

The committed rehearsal requires an explicit source path and Provider identity. It rejects
symlinks, non-regular files, group/world-readable source configuration, a missing Provider, a
missing active model alias, and an unavailable or rejecting OpenClaw validator.

The local execution used the installed OpenClaw CLI and its supported `OPENCLAW_CONFIG_PATH` and
`OPENCLAW_STATE_DIR` overrides:

1. read the live `0600` OpenClaw configuration once without printing any value;
2. create a `0700` temporary directory, a `0600` configuration copy, and an independent state
   directory;
3. validate the unmodified copy;
4. replace only the copied Provider endpoint, key with a synthetic `rgw_`-shaped non-credential,
   and active model alias;
5. verify that Provider/model protocol selection is unchanged and validate the migrated copy;
6. restore the original bytes to the copy, validate again, and require byte equality;
7. delete the temporary configuration and state automatically, then require the live source bytes
   to remain unchanged.

The fixed, value-free receipt was:

| Assertion | Result |
|---|---|
| Baseline copied configuration validates | PASS |
| Temporary endpoint/key/alias migration validates | PASS |
| Provider and model protocol selection is preserved | PASS |
| Temporary rollback restores the original bytes and validates | PASS |
| Live OpenClaw configuration remains unchanged | PASS |

No gateway restart, launchd mutation, external request, DNS change, server Config Version, real
Client Key issuance, production hostname change, or traffic change occurred. Therefore this proves
the client configuration transaction and rollback mechanics; it does **not** claim that a synthetic
key authenticated or that OpenClaw sent a successful live inference. Dedicated real Client Key
issuance plus non-streaming/SSE smoke checks remain cutover-window assertions after P12-08G1 has
validated the production graph.

## Review and regressions

The regression suite covers successful migration/rollback, broad source permissions, symlink input,
and an unknown Provider. The repository Full gate runs the suite without depending on OpenClaw being
installed in CI; the live Mac mini run separately used the installed validator.

Review found and corrected one initial evidence gap: schema validity alone did not prove that the
configured protocol remained stable. The final rehearsal now snapshots and compares both Provider
and model protocol selections across the temporary migration.

CC Switch remains intentionally untouched. Its eventual migration needs a new operator instruction
and a separate bounded maintenance transaction; P12-08F3 does not silently convert that deferral
into completion.
