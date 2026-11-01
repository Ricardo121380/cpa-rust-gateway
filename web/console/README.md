# CPAR console (`web/console`)

An independent management console for the gateway, built from scratch against
`docs/openapi/management-v1.json` and the backend source. It does not reuse or replace
`web/prism`; the gateway still embeds Prism at `/admin-ui/` until a separate decision switches it.

## Run

```bash
npm ci
npm run dev:mock      # in-memory API that follows the contract, no gateway needed
```

Against a real gateway (see the repository `CLAUDE.md` for building and starting it):

```bash
CONSOLE_GATEWAY=http://127.0.0.1:18181 npm run dev
```

The dev server proxies `/admin` to the management listener and rewrites `Origin` to the
listener's own address, which is the one browser origin the gateway admits. Log in with the
administrator account (`gateway admin-login init`) or a `mgmt_` key; the first login forces a
password change, which revokes every session.

## Gates

```bash
npm run typecheck && npm test && npm run build && npm run check
```

`scripts/check.mjs` enforces the serving constraints the gateway applies today, so the build can
be embedded without backend changes:

- `dist/` is exactly `index.html`, `assets/index.css`, `assets/main.js`, `assets/vendor.js`;
- nothing violates the management CSP (`script-src 'self'; style-src 'self'`): no inline
  `<script>`/`<style>`, no injected style elements, no external origins or fonts;
- no browser storage; no raw `fetch` outside `src/api/client.ts`;
- `src/api/schema.d.ts` and `src/api/ops.gen.ts` match the contract (`npm run gen:types`).

## Structure

- `src/api/` — typed client (`openapi-fetch`), header rules generated from the contract
  (`X-Management-Key`, CSRF, `X-Config-Version`, `If-Match`), in-memory session, mock transport.
- `src/app/` — shell, hash router, configuration-version bar (active → draft, fork, validate,
  publish), theme, toasts.
- `src/features/<area>/` — overview, requests, usage and billing, accounts, onboarding,
  upstreams, routing, access, versions, runtime, audit and backup, system.

Design direction combines three earlier prototypes (warm "ledger" palette, grouped sidebar,
row-level lists). Components come from shadcn/ui and the beUI and Rare UI registries
(`components.json`), 21st.dev Agent Elements (`src/components/agent-elements/`, MIT), and
Transitions.dev CSS (`src/styles/transitions-dev.css`). Licenses of the copied registry
components other than Agent Elements should be confirmed before release.

## Known backend observations

Filed as change requests, worked around on the frontend:

- `docs/change-requests/CR-CONSOLE-REQUESTS-CONCURRENCY-001.md` — concurrent request-ledger reads
  intermittently return 500.
- `docs/change-requests/CR-CONSOLE-VALIDATE-RESULT-001.md` — validation reports an invalid graph
  as 409 without reasons instead of `Validation{valid:false}`.
