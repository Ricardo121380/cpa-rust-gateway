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
