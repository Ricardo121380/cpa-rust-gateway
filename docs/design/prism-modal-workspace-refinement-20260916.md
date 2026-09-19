# Prism Modal and Workspace Refinement

Status: active implementation. Shared Sheet/channel authorization (`c2c_5477`), legacy OAuth renewal (`c2c_8a4d`), Provider workspace (`c2c_b63f`), account maintenance (`c2c_a74e`), and model/catalog lifecycle (`c2c_2f8c`, iteration 3) have approved checkpoints. Key/access, the remaining ledger rows, and final whole-application acceptance are still pending.

## Product frame

Prism is a control plane for people who operate multiple AI channels. Its main
job is to let an administrator make a safe change, understand its scope, and
return to the data workspace without learning the gateway's internal resource
model. This is an operations interface, not a landing page.

The generated UI Pro Max design-system search correctly identified Liquid Glass
as a compatible material, dense dashboard density, and low-motion operation.
It incorrectly suggested a marketing/testimonial layout and restaurant
typography on two narrowed queries. Those suggestions are rejected because they
do not match this product. The implementation uses the current Prism system
font stack, its Apple-blue action color and its existing ambient/lens treatment.

## Design system

### Material and color

| Role | Token direction | Purpose |
| --- | --- | --- |
| Ambient canvas | existing `--canvas` plus subtle Prism environment | Background only; never carries data. |
| Work surface | `--surface`, `--surface-2`, `--separator` | Solid, quiet tables and page workspace. |
| Overlay veil | new semantic `--modal-veil` | A restrained luminance veil; one backdrop blur budget. |
| Form panel | new semantic `--modal-surface` / `--modal-surface-raised` | A solid, layered panel that belongs to the glass chrome without making every field glass. |
| Primary action | existing Apple-blue tint token | One clear commit action per dialog. |
| Destructive action | existing critical status token | Only for destructive confirmation and never as a default secondary action. |
| Focus | new semantic `--focus-ring` | Visible 2px ring plus offset in both themes. |

No new external font, image, CDN, inline style or runtime color generation is
allowed. The current light and dark palette remains authoritative; components
consume semantic tokens rather than raw values.

### Type and density

- System display and body fonts stay in place to avoid a network dependency and
  preserve the native Apple-control-plane character.
- Dialog title: 20px / 26px, semibold. Supporting sentence: 14px / 20px.
- Field label: 13px / 18px, medium. Help, validation and metadata: 12px / 18px.
- Desktop panel width: 520px for a form or confirmation; 560px for a
  multi-section inspector. The content body, rather than the whole overlay,
  scrolls. Inspector remains a right-side sheet at about 520px.
- Mobile: 12px viewport inset, full-width panel, 44px minimum interactive
  targets and a persistent action row above safe-area padding.

### Interaction states

- `form`: title, one-sentence purpose, scrollable fields, inline validation,
  local error summary after a failed submit, and a stable action footer.
- `confirm`: concise impact statement, object label, optional destructive
  treatment, cancel on the left and explicit commit on the right.
- `inspector`: semantic summary header, grouped facts, optional actions in the
  footer; internal identifiers remain behind an explicit technical-details
  disclosure.
- `success`: concise receipt and a single Done action. Secret reveal remains
  one-time and is cleared before exit animation.
- `loading` and `empty`: retain the panel frame and say what is being loaded or
  what the administrator can do next instead of replacing the entire dialog.

## Shared layout

```
Desktop form / confirmation                Desktop inspector
┌──────────────────────────────┐           ┌────────── workspace ──────────┐
│ title                    [×] │           │                                │
│ one sentence of purpose      │           │                  ┌──────────┐  │
├──────────────────────────────┤           │                  │ title  × │  │
│ scrollable content           │           │                  │ summary  │  │
│ label                        │           │                  ├──────────┤  │
│ [ field ]                    │           │                  │ facts    │  │
│ inline guidance / error      │           │                  │ actions  │  │
├──────────────────────────────┤           │                  └──────────┘  │
│ Cancel          Save change  │           └────────────────────────────────┘
└──────────────────────────────┘

Mobile: the same order becomes a 12px-inset panel. The action footer remains
visible; it never requires horizontal scroll or an off-screen close affordance.
```

## Explicit anti-patterns

- No giant title, duplicate explanatory paragraphs, technical ID in a normal
  title, or a full-width action row when a normal footer is clearer.
- No nested Sheet, browser `window.confirm`, simultaneous primary buttons, or
  generic “Submit” wording.
- No large empty card inside another card merely to group one field.
- No blur, shadow or glass effect on every data cell; glass remains chrome and
  overlay material, data remains on solid surfaces.
- No tooltip-only, hover-only or icon-only critical action without an accessible
  name.

## Delivery sequence

1. Consolidate the shared overlay into one semantic `Sheet` contract with
   `form`, `confirm` and `inspector` variants, description, footer and optional
   tone. Remove duplicate sheet geometry from `app.css` and `modal.css`.
2. Convert account access, API-key, model connection and provider dialogs to
   the shared header/body/footer contract. These are the administrator's most
   frequent and most visible flows.
3. Convert destructive confirmations, inspectors and detailed configuration
   editors. Preserve the existing revision/CAS and service re-read behavior.
4. Apply the same primitives to billing, egress, catalog, runtime, audit and
   configuration workspaces; remove internal vocabulary from ordinary actions.
5. Audit long content, focus, reduced motion, dark theme and mobile behavior
   against the real local gateway before deployment.

## Accessibility and performance gates

- Visible focus and logical Tab order in every dialog; sticky footer cannot
  obscure focused fields or buttons.
- Labels remain associated with inputs; invalid fields have inline errors and
  `aria-describedby`. Failed submits focus an error summary only when one is
  present.
- Close returns focus to the opener; Escape respects pending/secret-reveal
  rules. Channel changes and close continue to cancel polling and clear
  transient input.
- Keep a single portal, one scrim, one `backdrop-filter`, no inline styles and
  no browser secret storage. Respect reduced-motion and reduced-transparency.
- Extract static footer/header markup and use narrowly scoped subscriptions;
  defer heavyweight inspectors or data visualizations only where a measured
  bundle cost justifies it. Do not split the fixed four-file build.

## Router-owned history admission

Sheet navigation protection is owned by React Router's `useBlocker`. A Sheet
does not write browser history, replace a HashRouter entry, or stop `popstate`
events. On a protected browser Back or Forward, the router restores the actual
indexed entry and keeps its own location state synchronized before the Sheet
offers either a discard choice or, for an irreversible write, resumes the busy
receipt view. This preserves intervening entries and the forward branch.

The regression uses three router-created entries around the API-key workspace,
holds the issuing request, and observes each departure plus compensating POP
for multi-step Back and Forward. After the receipt is complete it also verifies
Back → Keep editing and Back → Discard on a new dirty form. Test helpers may
not use raw document navigation to manufacture a history entry because such an
entry has no router index and cannot establish this invariant.

## Scope ledger

The shared Sheet currently services account authorization and maintenance,
provider and endpoint maintenance, model/routing workbenches, API-key flows,
billing, egress, catalog, runtime, audit and configuration confirmation. Each
of those call sites must be migrated or explicitly shown compatible with the
new contract before this refinement is considered complete.

| Caller | Intended variant | Dismissal / transaction compatibility | Layout migration | Acceptance evidence |
| --- | --- | --- | --- | --- |
| `components/ObjectInspector.tsx` | inspector | Close only; allowlisted facts | Migrated: description and inspector frame | Inspector E2E pending final pass. |
| `app/DraftDock.tsx` | confirm / receipt | One owned validation panel; publish acknowledgement before reread; no replay after lost response | Migrated: single loading/result frame and stable footer | `draft-publication-lifecycle.spec.ts` success, failure, lost-response and reread cases; C2C `c2c_2f8c` iteration 5. |
| `accounts/AddAccountDialog.tsx` | form / receipt | Imported secret and channel selection; explicit guarded close and cleanup | Migrated: stable form-associated footer and receipt action | `modal-foundation.spec.ts`; `modal-daily.spec.ts`. |
| `accounts/AuthorizationCodeDialog.tsx` | callback form / receipt | Channel task and cancellation complete before an accepted dismissal | Migrated: stable phase-specific footer, transient callback form and inline validation | Callback lifecycle E2E pending. |
| `accounts/KimiDeviceDialog.tsx` | device-progress / receipt | Device challenge, polling and cancellation | Migrated: stable footer; challenge stays visible during polling; cancellation precedes dismissal; an uncertain terminal poll exposes only a local exit | `channel-authorization.spec.ts` Kimi held/cancel/unresolved/Back cases. |
| `accounts/KiroDeviceDialog.tsx` | device-progress / form / receipt | Device challenge, polling and cancellation; optional organization values | Migrated: stable footer; native options form; cancellation precedes dismissal; an uncertain terminal poll exposes only a local exit | `channel-authorization.spec.ts` Kiro held/cancel/unresolved cases. |
| `accounts/GrokDeviceWizard.tsx` | device-progress / receipt | Native device identity and runtime receipt | Migrated: stable footer and cancel-before-dismiss | `native-accounts.spec.ts`; lifecycle race coverage pending. |
| `accounts/CredentialUpdateDialog.tsx` | form / receipt | Replacement material, exact owner/status admission, explicit recovery state and non-replayable receipt | Migrated: stable form footer; unsupported operational status requires a supported explicit choice | `account-actions.spec.ts` replacement, owner-race and read-only-status cases; C2C `c2c_a74e` iteration 6. |
| `accounts/AccountBatchDialog.tsx` | confirm / receipt | Destructive removal, exact captured ordinary/native targets and per-item result | Migrated: stable confirm footer and structured partial outcomes | `account-actions.spec.ts`, `managed-accounts.spec.ts`; real local gateway one-account disable/readback; C2C `c2c_a74e` iteration 6. |
| `accounts/NativeAccountDialog.tsx` | form / inspector | Native identity, SSO replacement and runtime apply | Migrated: guarded Back continuation, stable inspector/form/receipt footer | Native E2E pending. |
| `accounts/AccountRuntimePanel.tsx` | inspector / confirm / receipt | Runtime action state and exact ordinary/native maintenance admission | Migrated: captured runtime target, authority-specific resolution, stable owned editor and explicit uncertain result | `account-actions.spec.ts`, `resource-names.spec.ts` including pagination, error/retry, owner and held-write races; C2C `c2c_a74e` iteration 6. |
| `accounts/AccountsPage.tsx` | inspector / action menu | Account maintenance transitions | Migrated: inspector/action chooser and exact batch owner snapshot | `managed-accounts.spec.ts`, `account-actions.spec.ts`; C2C `c2c_a74e` iteration 6. |
| `access/IssueKeyDialog.tsx` | form / reveal receipt | One-time Client Key; stage-aware issuance and redacted lost-response reconciliation | Migrated: stable Create/Cancel then Copy/Done footer; secret clears on exit/session change | `modal-daily.spec.ts`, `key-access-lifecycle.spec.ts`; local mock accepted an allowed model and rejected a forbidden model. |
| `access/KeyPermissionsDialog.tsx` | form / receipt | Captured key/group/grants, private replacement group and button-only dirty state | Migrated: stable Save/Cancel footer and non-replayable outcome | `key-access-lifecycle.spec.ts` sibling isolation, partial move and preflight; local gateway applied edit/readback. |
| `access/AccessPage.tsx` / `GroupKeyDialog.tsx` | form / confirm / reveal receipt | Client keys, revocation and explicit-group draft signing | Daily signing/revocation migrated; advanced signing has owner-bound recovery and one-time reveal; legacy group routing remains | `key-access-lifecycle.spec.ts` including cancellation, replacement session, recovery GET and revocation; C2C `c2c_2f8c` iteration 5. |
| `upstreams/ProviderDialog.tsx` | form | Optional credential material; explicit close and busy state | Migrated: stable form-associated footer | `modal-daily.spec.ts`. |
| `upstreams/UpstreamsPage.tsx` | form / confirm | Provider tags and configuration revision | Chip dirty and confirmation migration; footer pending | Provider E2E pending. |
| `upstreams/SubresourcePanel.tsx` | form / inspector / confirm / receipt | Endpoint, account and binding configuration | Provider workspace checkpoint migrated: human-readable endpoint/account rows; channel-owned normal account entry; one active sheet/receipt action; semantic binding reconciliation; scheduling and raw credentials deliberately advanced | `subresource-crud.spec.ts` covers draft, active and uncertain application receipts, create/edit/delete, explicit connection, bounded scheduling values and unbound credential retention. |
| `upstreams/CredentialSheet.tsx` | inspector | Account identity and maintenance actions | Migrated: compact purpose header and stable Close footer | Daily inspector E2E pending. |
| `upstreams/OAuthWizard.tsx` | callback authorization / receipt | Existing Codex credential renewal, callback and cancellation | Migrated: mount-safe attempt identity, native callback form/footer, server-first cancellation, acknowledged receipt and explicit unknown-result recovery | `credential-oauth.spec.ts` covers compact status, local validation, production-envelope rejection/reconciliation, completion/reread failure, cache isolation, lost response, cancellation uncertainty/pending status, durable fallback and Back; C2C `c2c_8a4d` approved iteration 4. |
| `models/ConnectModelDialog.tsx` | form / receipt | Exact model ID and endpoint selection; explicit close and busy state | Migrated: stable form footer, pre-fork no-op probe, exact source and staged receipt | `modal-daily.spec.ts`, `model-catalog-lifecycle.spec.ts`; C2C `c2c_2f8c` iteration 3. |
| `models/ModelConnectionsDialog.tsx` | inspector / form / confirm / receipt | One captured source and last-enabled consequence | Migrated: one owned action, frozen fields during writes, structural preflight, stage-aware result | `model-catalog-lifecycle.spec.ts`; local embedded gateway source edit/readback; C2C `c2c_2f8c` iteration 3. |
| `models/ModelsPage.tsx` | form / confirm | Aliases, route setup and destructive remove | Description/confirmation migration partial | Model E2E pending. |
| `models/RouteWorkbench.tsx` | form / confirm | Candidates, aliases and routing revision | Compatibility adapter | Routing E2E pending. |
| `catalog/CatalogPage.tsx` / `UpstreamModelBrowser.tsx` | inspector / confirm / receipt | Exact directory target and model choice | Daily connection migrated: frozen evidence, bounded continuation, target-bound refresh and source revalidation; other diagnostics retain their existing view | `model-catalog-lifecycle.spec.ts`; local embedded gateway catalog activation; C2C `c2c_2f8c` iteration 3. |
| `billing/BillingPage.tsx` | form / confirm / receipt | Price catalog import | Compatibility adapter | Billing E2E pending. |
| `egress/EgressPage.tsx` | form / confirm / inspector | Host, port and CIDR policy | Chip dirty and confirmations migrated; footer pending | `modal-foundation.spec.ts`. |
| `egress/CompatibleProxyPanel.tsx` | form / confirm | Sealed proxy endpoints | Compatibility adapter | Egress E2E pending. |
| `runtime/PoolActionSheet.tsx` | confirm | Scheduler action | Migrated: stable footer, field-local validation and target-bound result | `account-actions.spec.ts`; C2C `c2c_a74e` iteration 6. |
| `monitoring/MonitoringPage.tsx` | inspector | Request attempt evidence | Compatibility adapter | Monitoring E2E pending. |
| `monitoring/RequestHistory.tsx` | inspector | Request evidence and usage | Compatibility adapter | Monitoring E2E pending. |
| `config-versions/LifecycleConfirmation.tsx` | confirm | Publish/rollback target and captured revision | Migrated: confirm action in shared footer | `draft-publication-lifecycle.spec.ts` publication CAS/reread/lost-response paths; C2C `c2c_2f8c` iteration 5. |
| `config-versions/ConfigurationDiff.tsx` | inspector | Configuration diff | Compatibility adapter | Diff E2E pending. |
| `config-versions/VersionsPage.tsx` | form / receipt | Draft lifecycle and revision | Compatibility adapter | Lifecycle E2E pending. |

The compatibility adapter is intentionally temporary. It keeps legacy Cancel,
Back and internal-link triggers routed through the shared discard policy while
the listed callers are converted. It may be removed only after every remaining
row is marked migrated and the targeted browser suite demonstrates the same
behavior.

## 2026-09-19 - Account maintenance checkpoint

The account batch preserves one selected ordinary/native authorization per exact
namespace and ID. Ordinary writes verify the captured Provider owner before
both preflight and draft mutation; native actions require an exact paginated
inventory match. Per-item outcomes distinguish skipped, rejected, saved draft,
saved but unapplied, applied, unexecuted and unconfirmed. Credential replacement
checks owner, kind, revision and observed status before writing, and requires an
explicit active/disabled decision for operational read-only statuses.

Runtime details verify an ordinary credential and its endpoint binding or a
native account by exact provider/ID before opening maintenance. Once admitted,
the editor holds its original account and revision; a background observation
cannot invert its action or discard a dirty/held form. Failed cached rereads
show an error and require an explicit fresh resolution. Chromium covered these
boundaries in 31 serial account cases; 42 unit files/340 tests, type check,
double-build SPA gate, management SPA gate, and three embedded UI Rust tests
passed. C2C `c2c_a74e` approved the full batch at iteration 6. An isolated
real gateway with loopback mock completed a one-account disable, active
application and UI readback. Official channel login, whole-app responsive
acceptance and production deployment are outside this checkpoint.

## 2026-09-19 - Provider workspace checkpoint

The provider detail is a deep-linkable workspace: selecting “接口与账号” stores
the Provider selection in the route so a successful configuration revision does
not close the administrator's current workspace. Normal endpoint and account
views are labelled rows that show protocol, host, account identity, access
method and status. The runtime binding matrix stays behind “高级调度配置”
because priority, weight and concurrency are operational policy rather than an
ordinary account action.

Endpoint editing now uses the shared Sheet's title, purpose, scrollable body
and form-associated footer. The model-list path and enable switch are progressive
disclosure. Connecting an account similarly starts with an explicit interface
and account selection, then exposes constrained priority, weight and concurrency
only under “调度设置”. Binding reconciliation is an inspector rather than a
normal flow: it states the mismatch without exposing an internal table as the
default page. Mobile turns its advanced matrices into labelled cards instead of
requiring horizontal scrolling.

After every endpoint, credential, binding or deletion write, the workspace
holds a non-secret receipt before changing its selected configuration or
reloading scoped inventories. The explicit Done/Review continuation adopts the
returned working version once, then refreshes. This prevents a remount from
hiding a durable outcome and keeps a fresh read within the correct revision
ownership boundary. The fixture's endpoint and credential 204 responses now
include the same revision receipt required by the actual task boundary.

### Transaction and account-action refinement

The provider panel owns one discriminated active action at a time: an
inspector, one form, binding reconciliation, a destructive confirmation or a
receipt. Normal “添加账号” leaves the provider workspace for the channel-owned
authorization/import chooser; raw material is available only from the explicit
“高级凭据维护” disclosure. This keeps ordinary OAuth and API-key flows from
asking an administrator to construct a Provider binding while preserving a
bounded maintenance path for an unbound imported credential.

Every endpoint, raw credential, binding and deletion mutation now has a
transaction receipt. A selected draft says it was saved but not applied. A
write begun from an active configuration automatically forks, validates and
publishes; a returned active version is an applied receipt. If a resource
response is lost or lacks its revision receipt, it becomes unconfirmed without
publication or replay. If publication fails after an acknowledged write, the
client reads the exact working version once, under the original task owner, and
reports saved-but-not-applied, durable-but-response-lost or unconfirmed. A
recovered active result must carry the same acknowledged revision; later
contents are never attributed to the initiating action. The fixture implements
the same single-version read declared by the management contract so the
uncertain-result branch exercises the real client operation rather than a
test-only substitute.

Only one active Provider action can exist at a time. While it is open, the
originating Provider workspace cannot be closed or switched; keyed panels also
prevent a retained receipt from being relabelled if the route changes. The
current serial Chromium evidence has 19 Provider workspace cases: runtime
observation failure and partial results, 1280/1440/390 pane fit, ordinary
endpoint/account and advanced credential maintenance, draft writes, active
automatic application, resource/publish response loss, missing revision
receipts, structural deletion preconditions, exact delete confirmation,
Provider-bound receipts and channel-owned account onboarding.

## 2026-09-20 - Model and catalog lifecycle checkpoint

The daily model inspector now owns one source edit or removal at a time. It
captures the exact public model, route candidate, sibling enabled state, and
source label before confirmation. An actual last enabled source is disclosed
before its removal or disablement also disables the public model; editing or
removing an already disabled source has no hidden model-status side effect.
Enabled, priority, and weight cannot change after an accepted Save, including
while publication is pending. A safe rejection before any resource write
restores editing; partial and uncertain writes instead retain a review-only
receipt. Model creation, route, candidate, and alias stages are counted rather
than presented as one atomic mutation. Active no-op connections are verified
against the source revision before any working draft is created.

Catalog connection requires an explicit endpoint and directory account. The
confirmation freezes its configuration revision, endpoint fields, directory
snapshot and observation time, and 1–20 exact model IDs. The selected account
supplies directory evidence; the configured runtime pool remains independent.
Pagination is explicit and bounded. A failed cursor blocks further selection
until the operator rereads, while each loaded page must share the same source
revision and snapshot. The admitted task rechecks its endpoint and catalog
evidence before the first model write. Refresh results remain bound to their
original endpoint/account and do not trigger a write replay.

Current checkpoint evidence: type check; 43 unit files / 347 tests; 24 serial
Chromium model/catalog/route cases and 56 related modal/account/Provider/channel
cases; double-build, four-file management SPA and three embedded UI Rust tests.
An isolated embedded gateway with a loopback mock applied a source weight 1→2,
read back 2, added two exact catalog models, and showed an active duplicate
connection left the configuration-version count at 4→4. EgoLite checked the
390px source inspector/editor with no document overflow. No real Provider
request or production state was changed. C2C `c2c_2f8c` approved the complete
model/catalog checkpoint at iteration 3. Key/access and remaining workspaces
are outside this checkpoint and remain active Goal work.

## 2026-09-20 - Key, access, and draft publication checkpoint

Daily key editing now captures the selected key, access group, grants, routes,
configuration revision and selection before any write. A changed key receives
a private replacement group, leaving siblings and disabled grants intact.
Grant submissions contain only the input schema's `route_id` and `enabled`;
the fixture rejects response-only fields. A missing active route fails before
group creation. Issuance and permission changes count acknowledged stages,
retain partial or uncertain receipts, and never replay a write after response
loss. Daily and advanced signing keep the one-time secret in transient reveal
state; a lost response is reconciled through a redacted record, with no
recoverable secret implied. Advanced signing names an explicit draft group,
propagates owner cancellation, and refuses recovery under a replacement
configuration or session. Revocation verifies the exact non-secret key record
before writing and retains an application receipt.

Draft validation owns a single loading Sheet from the accepted click until its
result or failure. Another editor cannot open over it. Publication captures
revision, expected active version and lifecycle event, then records an
acknowledged response before rereading the activity list. A failed reread
preserves the publication receipt; a lost publish response becomes a review
state without an automatic second write.

Current evidence: 43 unit files / 347 tests; 15 serial key/access/publication
Chromium cases, plus the earlier 22-case related run; type check, double-build
SPA and 152-operation/four-file management gate; three embedded UI Rust tests.
An isolated real gateway applied a key permission change, then a synthetic
restricted key returned 200 for an allowed model and 404 for a forbidden one
through a loopback mock. The latest embedded build and 390px key editor were
rechecked in EgoLite. No real Provider or production state was changed. C2C
`c2c_2f8c` approved the complete checkpoint at iteration 5. Group routing,
remaining advanced workspaces, whole-app acceptance and deployment remain
active Goal work.
