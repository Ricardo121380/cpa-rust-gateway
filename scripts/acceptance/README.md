# Prism local workflow acceptance

Run from the repository root after `cargo build -p gateway --bin gateway`:

```sh
python3 scripts/acceptance/prism-local-gateway.py /absolute/path/to/receipt-directory
```

This starts an isolated real gateway and an owned loopback TLS mock. It creates a temporary
state directory, private synthetic credentials and a private CA used only by the gateway process.
It never changes system trust, production state or real providers. Keep the controller running.
The printed URL serves the embedded production application; it is not the Vite fixture backend.

In another terminal:

```sh
python3 scripts/acceptance/prism-request-chain.py /absolute/path/to/receipt-directory
```

This verifies two-page catalog refresh without permission expansion, JSON/SSE success, HTTP failure,
truncated streaming, explicit TCP cancellation, durable terminal statistics and billing. It waits
for actual runtime recovery between intentional failures instead of disabling health protection.

For the UI portion, use EgoLite and the private initial administrator password file in the temporary
root. Complete first-login password change, select `Model-Second` in the real catalog, publish it,
and issue a key selecting only that model. Verify the select-all-current shortcut and clear it before
making the final single-model selection. Save that newly generated synthetic key as `ui-client-key`
in the private temporary root without printing it. Then run:

```sh
python3 scripts/acceptance/prism-ui-model-key.py /absolute/path/to/receipt-directory
```

It verifies the new key's exact permissions, unchanged old-key permissions, a denied cross-model
call and a real mock request whose missing price remains `unpriced`, not zero cost.

The controller supports a `restart-request` marker in its own temporary root after a rebuild.
Terminate only the controller belonging to this receipt; its cleanup stops its gateway and mock.
These checks do not prove real provider authorization or model-directory completeness.

## Legacy alias retirement

`prism-retire-legacy-aliases.py` performs an explicit plan/apply migration using
existing management credentials. First use an isolated production copy. `--base`,
`--origin`, `--credential-dir` and an unused `--receipt` path are required. Optional
`--upstream-names` reads a reviewed JSON object mapping exact upstream IDs to formal
names; it changes only the name field. Never place credential material in that file.

The initial invocation writes an owner-only plan. Apply that same plan with
`--apply-plan <plan-file> --target <new-draft-id>` and the same name mapping. The script
requires all six approved legacy aliases, preserves custom aliases, brackets reads
with the active revision and compares models/routes/candidates/groups/grants/keys
before publication and after readback. It does not edit original model IDs, account
credentials, historical requests or ledger rows, and makes no Provider calls.
A changed plan or existing target stops execution; inspect an interrupted draft
instead of replaying it. A successful local synthetic run is not production-copy
or real-channel acceptance.


### Exact-target catalog acceptance without background renewal

`gateway catalog-check` runs the same assembled metadata discovery as `/admin/catalog/refresh`,
but never starts listeners, inference, periodic maintenance or startup credential renewal.
Run it only on a fresh operator-owned state copy: use SQLite backup for control/admin stores,
copy the existing required credential files with owner-only permissions, and create
`<copy-state>/catalog-check.marker` containing exactly `isolated-metadata-only\n`.
Missing/symlink/nonmatching markers are rejected before opening the database.

```
gateway catalog-check --state-dir <absolute-copy-state> --credential-dir <absolute-copy-credentials> --endpoint-id <exact-endpoint> --credential-id <exact-account>
```

It writes only directory observations in the isolated copy and emits counts/closed errors, never
credentials or inference output. A stale access token is reported, never silently renewed from a
copied rotating grant. Re-create the copy from the live service after separately authorized renewal
if needed. This command is for production-copy acceptance; it does not replace the production HTTP
refresh workflow or prove official interactive authorization completed.

Alias retirement also accepts `--catalog-paths <reviewed-json>` for exact endpoint IDs and model-list paths. These paths join the static graph fingerprint and expected readback; normal request timestamps do not invalidate the reviewed configuration plan. Infer no path from a provider name: verify the base path and its documented model-list endpoint first.
