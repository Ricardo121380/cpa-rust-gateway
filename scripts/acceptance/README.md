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

## M3 V2 recovery and operations (EgoLite)

Use one owned EgoLite TaskSpace for the task. Its `p1` is the real synthetic gateway;
its `p2` is the optional local Vite fixture (`VITE_PRISM_FIXTURES=1`). Never point
fault-injection scripts at production, use real credentials, or claim fixture authorization
as an official login. Preserve the same space between scripts; no Playwright browser is used.

After the real first-login password change, save only the synthetic QA password in
`<temporary-root>/qa-password` with mode 0600. Keep the existing controller running. Run
`prism-request-chain.py` once and `prism-m3-seed.py <receipt-directory>` once; this gives
57 requests and 54 ledger rows for the fixed pagination assertions. Re-running the seed
changes that fixture and invalidates the exact-count assertions.

Run each `.mjs` through `ego-browser nodejs`, prepending a JavaScript assignment:

```js
globalThis.PRISM_M3 = { spaceId: 12, receipts: "/absolute/path/to/receipt-directory" };
// Follow with the exact contents of the selected script; use the task's actual space ID.
```

- `prism-m3-recovery.mjs`: true gateway reads with local response faults; first page,
  next cursor, background refresh, render exception, attempt detail, target deep link and
  overview time ownership. It reloads and signs in using the private synthetic password.
- `prism-m3-monitoring.mjs`: initial and background failures in ledger/failure tabs.
- `prism-m3-diagnostic-links.mjs`: click all four persisted-attempt links and verify
  exact account, expanded provider, scoped catalog and runtime diagnosis destinations.
- `prism-m3-matrix.mjs`: eight workspaces × three sizes × two themes, real gateway.
- `prism-m3-dialogs.mjs`: account chooser/model/key dialogs, keyboard, long input, preferences.
- `prism-m3-batch.mjs`: first real draft mutation, second locally injected conflict,
  third unexecuted; no publication. Run after other data-dependent matrix checks.
- `prism-m3-unlock.mjs`: six login layouts, real logout, no retained password or protected content.
- `prism-session-navigation.mjs`: separate `PRISM_SESSION_QA={spaceId,root,origin,mode,output}`;
  `mode` is `immediate`, `settled` or `held`. Run each once on the same synthetic gateway.
- `prism-m3-native.mjs`: supplemental fixture-only unknown write/409/pending runtime apply;
  additionally set `fixtureOrigin` to the local Vite URL. These are synthetic management
  responses, not upstream authorization or production evidence.
- `prism-m3-ui-timing.mjs`: fixture UI comparison; set `baselineOrigin` and `candidateOrigin`
  to two owned loopback Vite servers. Uses the same p2, viewport and data, one warm-up and
  five samples. It does not measure gateway latency or production Core Web Vitals.

Do not run browser scripts against p1 and p2 simultaneously: background rendering/timer
throttling makes retry and animation measurements unreliable. Store script failures as well
as successes. Keep raw screenshots locally and record which representative images are tracked.
At completion, finish only the owned TaskSpace and stop only the owned fixture/controller.

## Agent multi-turn regression

`python3 scripts/test-agent-roundtrip.py` launches and cleans up a fresh owned gateway/mock,
runs four three-turn flows (JSON/SSE × client-managed/stored history), with two tool-result
continuations each, then the
existing error/truncation/cancellation/ledger scenarios. It is part of `scripts/check.sh` after
the gateway build. The client replays the gateway's exact output items (including reasoning and
tool status) instead of manually deleting unsupported history. The mock verifies the next
request, and authenticated decode failures must be counted without attempts or billed usage.
Successful temporary fixtures are removed; failed runs retain their private controller log,
receipt and synthetic state for diagnosis. The fixture explicitly opts its generic Responses
candidate into stored-response support; production capabilities are unchanged.
There are zero external Provider calls. For an already running owned fixture, run
`python3 scripts/acceptance/prism-agent-roundtrip.py <receipt-directory>` once.

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
