# Independent live evidence verification

One read-only scout examined 139 public assets (including decompressed logs) and the fixed product diff through `f346a03f912b150006a288c38087550bb3d04cb0`. It did not read private output, credentials, or the VPS; it did not execute tests, contact Providers, or change files.

The source/artifact/guard hashes agree across the repaired artifact, upload, restart, and final runtime receipts. Twelve recorded implementation file hashes match the corresponding committed blobs. The fixed-source Full log independently totals Rust 1501 passed, zero failed, 12 ignored, and ends with `check: full passed`; its byte/hash archive matches the run receipt. Fixed-source process replay is stock public HTTP with synthetic TLS loopback; it does not contain real Provider calls.

Restart PASS is limited to configuration, process/artifact binding and 65 seconds of disabled background activity. Cleanup PASS is limited to stopping the owned candidate and builder and checking production invariants. Neither means real inference PASS.

The latest management HTTP200 carries a failed Pin receipt: one attempt, upstream_sent true, no response start, stage http_transport; the correlated closed Provider diagnostic is HTTP426/Unrecognized. Catalog GET is one, protected Pin attempt is one, basics are zero, and the other five combinations are NOT_RUN. The earlier requested/started pair is a known no-send management rejection. Two DNS and two TCP audit records cannot be reported as two server HTTP receives. Basic Usage and ledger qualification remains NOT_RUN; billing amount remains NOT_PROVEN.

Original Clippy, release-environment, fixture, snapshot-reader, Pin, and guard failures remain present. The latest successful Full explicitly uses the repository's default development environment; it does not turn the retained release-environment failure into a passing release run. Generic artifact/metadata names still refer to the old 98febf05 candidate; f346a03f-specific files are the repaired candidate authority.

Pattern and structured-field scans of these public assets found no obvious real email, JWT, Bearer value, private key, secret assignment, or raw account identity. A `pin_keys` value of `str` was a type-description false positive. This is a bounded inspection, not an absolute no-leak claim. Fixture-generated synthetic material is not a real credential.

The scout did not independently hash the native binary, inspect private budgets/logs/account bytes, or establish current VPS state. Those boundaries are covered by the root's separately bound remote receipts; public receipt consistency alone cannot prove them.
