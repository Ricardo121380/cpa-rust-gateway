# Prism daily-workspace alignment

## Scope and reference review

This iteration reviewed CPA-Manager-Plus at `376b1e8`, CLIProxyAPI at
`7bbfeaf`, and Cli-Proxy-API-Management-Center at `c12997e` against the
delivered Prism application and its live management contract. The references
establish the operator workflow, rather than an implementation to copy:

| Reference pattern | Prism mapping |
| --- | --- |
| CPAMP provider, account/OAuth, monitoring, usage, configuration and system workspaces | Prism keeps the daily path as dashboard, accounts, providers, models, API keys, request logs, usage, and settings. Account authorization/import and provider connections remain first-class actions. |
| Management Center provider workbench, auth-file inventory, OAuth, quota and logs | Prism uses compact provider rows with a detail workspace, grouped account identity/authorization rows, native authorization entries, exact model catalogues, restricted keys, and snapshot-backed request records. |
| CLIProxyAPI runtime semantics | Prism continues to use its own authoritative management and data contracts. It does not copy YAML editing, secret exports, synthetic provider status, or a separate analytics service. |

The result retains the existing 14 deep-linkable routes. Egress, runtime,
configuration history, and audit/backup are kept as secondary Settings
workspace pages; they are no longer a normal operator's primary navigation
context.

## Changes

- Removed the visible configuration-revision indicator from the global chrome.
  Version selection still bootstraps silently before version-scoped resource
  reads, so the presentation change cannot leave accounts, providers, models,
  or keys in a permanent loading state.
- Restored one coherent data-surface material: glass remains reserved for the
  rail, top bar, and draft dock; readable cards, tables, metrics, and
  inspectors share a restrained satin surface with borders and elevation.
  This replaces the late global CSS override that made every centre panel
  transparent and flat.
- Kept Settings as the entry for advanced maintenance and made its internal
  workspace navigation consistently reachable from Settings and every
  advanced deep link. An archived or draft configuration now displays a global
  text notice across the workspaces, while an active revision remains out of
  the day-to-day chrome.
- Clarified the account entry point as **Authorize / import account**. The
  dialog keeps real channel-specific OAuth/device/SSO and credential-import
  flows; it does not create placeholders or collect secrets in browser
  storage.
- Corrected request-observation documentation and dashboard copy. Persisted
  request terminals supply real outcomes, buckets, duration, first-content
  timing, P50 and P95; process counters remain explicitly cumulative.
- Fixed a request-log deep-link defect. Applying a model, channel, account,
  key, or result filter now preserves an exact `from_ms`/`to_ms` range from a
  dashboard link. Selecting a new range preset intentionally replaces it, and
  the request bucket size now follows the actual resolved time span. A
  non-preset deep link is labelled as its current exact range until an operator
  intentionally selects a relative preset.
- Added a real filtered-empty recovery state for Providers and distinguish a
  failed topology read from a pending one in both Providers and Models. The
  topology reader now sequences bounded management reads and safely retries a
  transient read-capacity rejection.
- Applied the same bounded retry policy to safe request-filter, ledger, and
  failure reads. The shared client also serializes management GETs to respect
  the gateway's bounded blocking-read worker. Writes and conflict responses
  are still never replayed.

## Verification

An isolated real local gateway and owned loopback TLS provider mock were used;
no production state or real Provider inference was touched.

- Focused Vitest regression: navigation plus the two exact-range preservation
  cases passed.
- Type check and production Vite build passed. The embedded output remains the
  required four files: `index.html`, `assets/main.js`, `assets/vendor.js`, and
  `assets/index.css`.
- `cargo build -p gateway --bin gateway` passed after embedding the rebuilt
  Prism assets.
- The real local request-chain acceptance passed: two-page catalogue refresh
  did not widen serving permissions; JSON/SSE success, failure, truncated
  streaming, cancellation, terminal observations, latency summaries, and
  billing materialization were observed through the gateway. The mock recorded
  zero real Provider calls.
- EgoLite verified the rebuilt embedded application at 1440-wide light,
  1280×720 dark, and 390×844 mobile layouts. The dashboard showed persisted
  P50/P95 and first-content values, the filter submission retained its exact
  time range, and Accounts, Providers, Models, and Settings all had coherent
  navigation and no page-level horizontal overflow at the mobile breakpoint.

## Follow-up

This report covers the implementation batch and local acceptance. The next
step is the existing signed release gate and deployment to the approved Oracle
CPAR service, followed by a human check of the public administrator login and
the revised daily workspaces.
