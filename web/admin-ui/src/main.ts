/**
 * P10-05 protected draft-management workspace.
 *
 * Management authentication values exist only in this module's live closure. The SPA never uses
 * browser storage, URL parameters, request-body previews, or response rendering that exposes a
 * Credential Secret. The generated client is constructed only after the operator explicitly
 * supplies a Management Key and CSRF token for this page lifetime.
 */
import {
  ManagementApi,
  type ManagementOperationName,
  type ManagementRequest,
} from "./generated/management-client.js";

type ResourceKind =
  | "egress"
  | "upstream"
  | "endpoint"
  | "credential"
  | "binding"
  | "publicModel"
  | "modelAlias"
  | "route"
  | "candidate"
  | "accessGroup"
  | "accessGroupRoute"
  | "clientKey";
type ResourceAction = "list" | "get" | "create" | "update" | "delete" | "issue" | "revoke" | "validate";
type LifecycleAction = "list" | "get" | "create" | "validate" | "publish" | "rollback" | "audit";
type BackupAction = "sourcePreflight" | "restorePreflight" | "restore";

type InMemorySession = Readonly<{
  managementKey: string;
  csrfToken: string;
}>;

let session: InMemorySession | undefined;
let managementApi: ManagementApi | undefined;

const resourceActions: Readonly<Record<ResourceKind, readonly ResourceAction[]>> = {
  egress: ["list", "get", "create", "update", "delete"],
  upstream: ["list", "get", "create", "update", "delete"],
  endpoint: ["get", "create", "update", "delete"],
  credential: ["get", "create", "update", "delete"],
  binding: ["list", "create"],
  publicModel: ["list", "get", "create", "update", "delete"],
  modelAlias: ["create"],
  route: ["get", "create", "update", "delete", "validate"],
  candidate: ["create"],
  accessGroup: ["list", "get", "create", "update", "delete"],
  accessGroupRoute: ["list", "create"],
  clientKey: ["list", "get", "issue", "update", "revoke"],
};

const resourceTemplates: Readonly<Record<ResourceKind, string>> = {
  egress: JSON.stringify(
    {
      id: "provider-egress",
      name: "Provider egress",
      allowed_schemes: ["https"],
      allowed_hosts: ["api.example.test"],
      allowed_ports: [443],
      allowed_cidrs: [],
      redirect_mode: "deny",
      max_redirects: 0,
    },
    null,
    2,
  ),
  upstream: JSON.stringify(
    {
      id: "provider-a",
      name: "Provider A",
      kind: "openai-compatible",
      enabled: true,
      tags: [],
      egress_policy_id: "provider-egress",
    },
    null,
    2,
  ),
  endpoint: JSON.stringify(
    {
      id: "provider-a-responses",
      adapter_id: "openai-compatible.responses",
      api_format: "openai/responses",
      base_url: "https://api.example.test/v1",
      inference_path: "/responses",
      models_path: "/models",
      transport: "https",
      enabled: true,
    },
    null,
    2,
  ),
  credential: JSON.stringify(
    {
      id: "provider-a-key-1",
      kind: "api_key",
      secret: "",
      status: "active",
    },
    null,
    2,
  ),
  binding: JSON.stringify(
    {
      credential_id: "provider-a-key-1",
      enabled: true,
      priority: 0,
      weight: 100,
      concurrency: 1,
    },
    null,
    2,
  ),
  publicModel: JSON.stringify(
    {
      id: "model-minimax-m3",
      model_name: "minimax-m3",
      status: "active",
      display_name: "MiniMax M3",
      capabilities: { streaming: true },
    },
    null,
    2,
  ),
  modelAlias: JSON.stringify({ alias: "minimax-m3-latest" }, null, 2),
  route: JSON.stringify(
    {
      id: "route-minimax-m3",
      policy: "smooth_weighted_round_robin",
      max_attempts: 2,
      bootstrap_timeout_ms: 2000,
    },
    null,
    2,
  ),
  candidate: JSON.stringify(
    {
      id: "candidate-minimax-m3",
      endpoint_id: "provider-a-responses",
      upstream_model: "minimax-m3",
      credential_scope: "all_active",
      transform_mode: "canonical",
      enabled: true,
      priority: 0,
      weight: 100,
      capability_override: {},
    },
    null,
    2,
  ),
  accessGroup: JSON.stringify(
    { id: "group-minimax-m3", name: "MiniMax M3", status: "active", limits: { rpm: 60 } },
    null,
    2,
  ),
  accessGroupRoute: JSON.stringify({ route_id: "route-minimax-m3", enabled: true }, null, 2),
  clientKey: JSON.stringify(
    {
      id: "client-minimax-m3",
      access_group_id: "group-minimax-m3",
      status: "active",
      expires_at_ms: null,
    },
    null,
    2,
  ),
};

function applicationMarkup(): string {
  return `
    <div class="shell">
      <aside class="sidebar">
        <p class="brand">CPA Rust Gateway<span>Management plane</span></p>
        <nav class="navigation" aria-label="Management sections">
          <a href="#upstreams">Upstreams</a>
          <a aria-current="page" href="#routing">Routing and access</a>
          <a href="#runtime">Runtime</a>
          <a href="#configuration">Configuration</a>
          <a href="#backup">Backup and restore</a>
        </nav>
        <p class="sidebar-note">P10-08 adds encrypted backup preflight and one-time, empty-target restore controls. Artifact creation and both Backup/Master Keys stay outside this page.</p>
      </aside>
      <main class="content">
        <header>
          <p class="eyebrow">P10-08 · protected management workspace</p>
          <h1>Upstreams, routing and access</h1>
          <p class="lead">Manage draft resources, publish only a validated draft, and inspect safe runtime projections. No view can expose a Secret, select a Provider, send an inference request or complete recovery.</p>
        </header>

        <section class="panel" aria-labelledby="session-heading">
          <div class="panel-heading">
            <div><p class="eyebrow">Page-local session</p><h2 id="session-heading">Connect management client</h2></div>
            <p id="connection-status" class="status" role="status">Not connected</p>
          </div>
          <form id="session-form" class="form-grid" novalidate>
            <label>Management Key<input id="management-key" name="management-key" type="password" autocomplete="off" required></label>
            <label>CSRF Token<input id="csrf-token" name="csrf-token" type="password" autocomplete="off" required></label>
            <label>Config Version<input id="config-version" name="config-version" value="draft-p10" required></label>
            <label>Current revision<input id="config-revision" name="config-revision" value="rev-0" pattern="rev-[0-9]+" required></label>
            <div class="form-actions"><button type="submit">Connect in memory</button></div>
          </form>
          <p class="notice">Keys and CSRF tokens are never written to browser storage, URLs, the result pane or request previews. Refreshing this page clears them.</p>
        </section>

        <section class="panel" id="upstreams" aria-labelledby="resource-heading">
          <div class="panel-heading"><div><p class="eyebrow">Draft resources</p><h2 id="resource-heading">CRUD, routing and access controls</h2></div></div>
          <form id="resource-form" class="form-grid" novalidate>
            <label>Resource<select id="resource-kind" name="resource-kind">
              <option value="egress">Egress policy</option><option value="upstream">Upstream</option>
              <option value="endpoint">Endpoint</option><option value="credential">Credential</option>
              <option value="binding">Endpoint credential binding</option>
              <option value="publicModel">Public model</option><option value="modelAlias">Model alias</option>
              <option value="route">Route</option><option value="candidate">Route candidate</option>
              <option value="accessGroup">Access group</option><option value="accessGroupRoute">Access-group route grant</option>
              <option value="clientKey">Client Key</option>
            </select></label>
            <label>Action<select id="resource-action" name="resource-action"></select></label>
            <label id="resource-id-field"><span id="resource-id-label">Resource ID</span><input id="resource-id" name="resource-id" autocomplete="off"></label>
            <label id="resource-scope-field"><span id="resource-scope-label">Parent resource ID</span><input id="resource-scope-id" name="resource-scope-id" autocomplete="off"></label>
            <label class="wide">Resource JSON<textarea id="resource-json" name="resource-json" rows="13" spellcheck="false"></textarea></label>
            <div class="form-actions"><button type="submit">Run protected operation</button></div>
          </form>
          <p class="muted" id="routing">Credential Secrets remain write-only. Client Keys are displayed only after a successful issue operation, in a separate transient pane; every other result is metadata only.</p>
        </section>

        <section class="panel" aria-labelledby="workflow-heading">
          <div class="panel-heading"><div><p class="eyebrow">Bounded workflows</p><h2 id="workflow-heading">Endpoint and OAuth controls</h2></div></div>
          <div class="workflow-grid">
            <form id="endpoint-workflow-form" class="form-grid" novalidate>
              <label>Endpoint ID<input id="workflow-endpoint-id" autocomplete="off" required></label>
              <label>Test mode<select id="endpoint-test-mode"><option value="non_streaming">Non-streaming</option><option value="sse">SSE</option></select></label>
              <label>Endpoint action<select id="catalog-action"><option value="test">Test endpoint</option><option value="preview">Preview Catalog diff</option><option value="apply">Apply Catalog diff</option></select></label>
              <div class="form-actions"><button type="submit">Run endpoint workflow</button></div>
            </form>
            <form id="oauth-workflow-form" class="form-grid" novalidate>
              <label>Credential ID<input id="workflow-credential-id" autocomplete="off" required></label>
              <label>OAuth action<select id="oauth-action"><option value="start">Start</option><option value="status">Check status</option><option value="cancel">Cancel</option></select></label>
              <div class="form-actions"><button type="submit">Run OAuth workflow</button></div>
            </form>
          </div>
          <p class="muted">Test results expose only safe outcome/status classes. Catalog apply uses the displayed revision; stale revisions fail before an injected workflow is called.</p>
        </section>

        <section class="panel" id="runtime" aria-labelledby="runtime-heading">
          <div class="panel-heading"><div><p class="eyebrow">Runtime observations</p><h2 id="runtime-heading">Catalog, availability and controlled recovery</h2></div></div>
          <div class="workflow-grid">
            <form id="runtime-observation-form" class="form-grid" novalidate>
              <fieldset class="form-actions"><legend>Safe observations</legend><button id="read-catalog-status" type="button">Read Catalog status</button><button id="read-runtime-availability" type="button">Read runtime availability</button></fieldset>
              <p class="muted">Catalog and availability show only binding IDs and closed freshness/Health/Quota/403 categories. They do not send a Provider request.</p>
            </form>
            <form id="quota-recovery-form" class="form-grid" novalidate>
              <label>Endpoint ID<input id="runtime-endpoint-id" autocomplete="off" required></label>
              <label>Credential ID<input id="runtime-credential-id" autocomplete="off" required></label>
              <label>Optional upstream model<input id="runtime-upstream-model" autocomplete="off" maxlength="256"></label>
              <div class="form-actions"><button type="submit">Request controlled quota recovery</button></div>
              <p class="muted">This records a bounded controller request only. It does not probe a Provider, clear a 403 state, change a Credential or declare recovery complete.</p>
            </form>
            <form id="route-explain-form" class="form-grid" novalidate>
              <label>Route ID<input id="runtime-route-id" autocomplete="off" required></label>
              <label>Requested model<input id="runtime-requested-model" autocomplete="off" maxlength="256" required></label>
              <label>Protocol<select id="runtime-protocol"><option value="openai_chat_completions">OpenAI Chat Completions</option><option value="openai_responses">OpenAI Responses</option><option value="anthropic_messages">Anthropic Messages</option></select></label>
              <div class="form-actions"><button type="submit">Explain Route</button></div>
              <p class="muted">Explain is a fixed-time projection. It does not acquire a Credential lease or advance a scheduling cursor.</p>
            </form>
            <form id="request-attempts-form" class="form-grid" novalidate>
              <label>Request ID<input id="runtime-request-id" autocomplete="off" required></label>
              <div class="form-actions"><button type="submit">Read value-free attempts</button></div>
              <p class="muted">Attempts omit Provider URLs, headers, bodies, model values, timing and raw diagnostics.</p>
            </form>
          </div>
        </section>

        <section class="panel" id="configuration" aria-labelledby="configuration-heading">
          <div class="panel-heading"><div><p class="eyebrow">Configuration lifecycle</p><h2 id="configuration-heading">Validate, publish and recover one predecessor</h2></div></div>
          <form id="configuration-lifecycle-form" class="form-grid" novalidate>
            <label>Lifecycle action<select id="configuration-lifecycle-action" name="configuration-lifecycle-action">
              <option value="list">List Config Versions</option><option value="get">Read Config Version</option>
              <option value="create">Create draft</option><option value="validate">Validate draft</option>
              <option value="publish">Publish draft</option><option value="rollback">Rollback retained predecessor</option>
              <option value="audit">Read lifecycle audit</option>
            </select></label>
            <label id="configuration-version-field">Config Version ID<input id="configuration-version-id" autocomplete="off" value="draft-p10"></label>
            <label id="configuration-parent-field">Optional parent Version ID<input id="configuration-parent-id" autocomplete="off"></label>
            <label class="wide" id="configuration-description-field">Draft description<textarea id="configuration-description" rows="4" maxlength="1024" spellcheck="false">P10 management draft</textarea></label>
            <div class="form-actions"><button type="submit">Run lifecycle operation</button></div>
          </form>
          <p class="muted">Validation changes no state. Publish requires the displayed revision and can activate only a draft; rollback can restore only P2's retained predecessor. Audit contains bounded metadata, never compiler diagnostics, Secrets, keys, URLs or request material.</p>
        </section>

        <section class="panel" id="backup" aria-labelledby="backup-heading">
          <div class="panel-heading"><div><p class="eyebrow">Encrypted recovery</p><h2 id="backup-heading">Preflight and empty-target restore</h2></div></div>
          <form id="backup-form" class="form-grid" novalidate>
            <label>Backup action<select id="backup-action" name="backup-action">
              <option value="sourcePreflight">Read configured backup preflight</option>
              <option value="restorePreflight">Preflight selected encrypted artifact</option>
              <option value="restore">Restore selected artifact into configured empty target</option>
            </select></label>
            <label id="backup-artifact-field">Encrypted artifact<input id="backup-artifact" name="backup-artifact" type="file" accept="application/octet-stream,.cpa-backup" autocomplete="off"></label>
            <div class="form-actions"><button type="submit">Run backup operation</button></div>
          </form>
          <p class="muted">The selected artifact is passed once directly to the generated client, then immediately cleared. This page neither reads nor renders artifact bytes, accepts a Backup Key, stores a Master Key, creates a download, chooses a restore path, or replaces an existing database.</p>
        </section>

        <section class="panel" aria-labelledby="result-heading">
          <div class="panel-heading"><div><p class="eyebrow">Safe result</p><h2 id="result-heading">Operation response</h2></div></div>
          <pre id="operation-result" class="result" aria-live="polite">No operation has run.</pre>
        </section>

        <section id="issued-client-key-panel" class="panel" aria-labelledby="issued-client-key-heading" hidden>
          <div class="panel-heading"><div><p class="eyebrow">Display once</p><h2 id="issued-client-key-heading">New Client Key</h2></div><button id="clear-issued-client-key" type="button">Clear now</button></div>
          <p class="notice">Copy this Client Key now. It is never available from a later read, list, update or revoke result, and the page clears it before every subsequent operation or reload.</p>
          <pre id="issued-client-key" class="result" aria-live="assertive"></pre>
        </section>
      </main>
    </div>`;
}

function element<T extends HTMLElement>(id: string): T {
  const found = document.querySelector<T>(`#${id}`);
  if (found === null) {
    throw new Error("management application element is missing");
  }
  return found;
}

function currentResourceKind(): ResourceKind {
  return element<HTMLSelectElement>("resource-kind").value as ResourceKind;
}

function currentResourceAction(): ResourceAction {
  return element<HTMLSelectElement>("resource-action").value as ResourceAction;
}

function setResult(value: unknown): void {
  element<HTMLElement>("operation-result").textContent = JSON.stringify(value, null, 2);
}

function clearIssuedClientKey(): void {
  element<HTMLElement>("issued-client-key").textContent = "";
  element<HTMLElement>("issued-client-key-panel").hidden = true;
}

function setFailure(message: string): void {
  clearIssuedClientKey();
  setResult({ ok: false, message });
}

function requiredValue(id: string): string {
  const value = element<HTMLInputElement>(id).value.trim();
  if (value.length === 0) {
    throw new Error("a required management input is missing");
  }
  return value;
}

function optionalValue(id: string): string | null {
  const value = element<HTMLInputElement>(id).value.trim();
  return value.length === 0 ? null : value;
}

function headers(includeRevision: boolean): Record<string, string> {
  const values: Record<string, string> = { "X-Config-Version": requiredValue("config-version") };
  if (includeRevision) {
    values["If-Match"] = requiredValue("config-revision");
  }
  return values;
}

function lifecycleHeaders(includeRevision: boolean): Record<string, string> {
  return includeRevision ? { "If-Match": requiredValue("config-revision") } : {};
}

function parseResourceBody(): unknown {
  try {
    return JSON.parse(element<HTMLTextAreaElement>("resource-json").value) as unknown;
  } catch {
    throw new Error("resource JSON must be valid");
  }
}

function operationForResource(kind: ResourceKind, action: ResourceAction): ManagementOperationName {
  const operations: Readonly<Record<ResourceKind, Readonly<Partial<Record<ResourceAction, ManagementOperationName>>>>> = {
    egress: { list: "listEgressPolicies", get: "getEgressPolicy", create: "createEgressPolicy", update: "updateEgressPolicy", delete: "deleteEgressPolicy" },
    upstream: { list: "listUpstreams", get: "getUpstream", create: "createUpstream", update: "updateUpstream", delete: "deleteUpstream" },
    endpoint: { get: "getEndpoint", create: "createEndpoint", update: "updateEndpoint", delete: "deleteEndpoint" },
    credential: { get: "getCredential", create: "createCredential", update: "updateCredential", delete: "deleteCredential" },
    binding: { list: "listEndpointCredentialBindings", create: "createEndpointCredentialBinding" },
    publicModel: { list: "listPublicModels", get: "getPublicModel", create: "createPublicModel", update: "updatePublicModel", delete: "deletePublicModel" },
    modelAlias: { create: "createModelAlias" },
    route: { get: "getRoute", create: "createRoute", update: "updateRoute", delete: "deleteRoute", validate: "validateRoute" },
    candidate: { create: "createRouteCandidate" },
    accessGroup: { list: "listAccessGroups", get: "getAccessGroup", create: "createAccessGroup", update: "updateAccessGroup", delete: "deleteAccessGroup" },
    accessGroupRoute: { list: "listAccessGroupRoutes", create: "grantAccessGroupRoute" },
    clientKey: { list: "listClientKeys", get: "getClientKey", issue: "issueClientKey", update: "updateClientKey", revoke: "revokeClientKey" },
  };
  const operation = operations[kind][action];
  if (operation === undefined) {
    throw new Error("that resource action is not available");
  }
  return operation;
}

function resourceRequest(kind: ResourceKind, action: ResourceAction): ManagementRequest {
  const scopeId = element<HTMLInputElement>("resource-scope-id").value.trim();
  const mutates = action === "create" || action === "update" || action === "delete" || action === "issue" || action === "revoke";
  const request: { path: Record<string, string>; headers: Record<string, string>; body?: unknown } = {
    path: {},
    headers: headers(mutates),
  };
  if (kind === "egress" && action !== "list" && action !== "create") {
    request.path.egress_policy_id = requiredValue("resource-id");
  }
  if (kind === "upstream" && action !== "list" && action !== "create") {
    request.path.upstream_id = requiredValue("resource-id");
  }
  if (kind === "endpoint") {
    if (action === "create") {
      if (scopeId.length === 0) {
        throw new Error("an owning Upstream ID is required to create an Endpoint");
      }
      request.path.upstream_id = scopeId;
    } else {
      request.path.endpoint_id = requiredValue("resource-id");
    }
  }
  if (kind === "credential") {
    if (action === "create") {
      if (scopeId.length === 0) {
        throw new Error("an owning Upstream ID is required to create a Credential");
      }
      request.path.upstream_id = scopeId;
    } else {
      request.path.credential_id = requiredValue("resource-id");
    }
  }
  if (kind === "binding") {
    request.path.endpoint_id = requiredValue("resource-id");
  }
  if (kind === "publicModel" && action !== "list" && action !== "create") {
    request.path.public_model_id = requiredValue("resource-id");
  }
  if (kind === "modelAlias") {
    request.path.public_model_id = requiredScopeId("Public Model ID", scopeId);
  }
  if (kind === "route") {
    if (action === "create") {
      request.path.public_model_id = requiredScopeId("Public Model ID", scopeId);
    } else {
      request.path.route_id = requiredValue("resource-id");
    }
  }
  if (kind === "candidate") {
    request.path.route_id = requiredScopeId("Route ID", scopeId);
  }
  if (kind === "accessGroup" && action !== "list" && action !== "create") {
    request.path.access_group_id = requiredValue("resource-id");
  }
  if (kind === "accessGroupRoute") {
    request.path.access_group_id = requiredScopeId("Access Group ID", scopeId);
  }
  if (kind === "clientKey" && action !== "list" && action !== "issue") {
    request.path.client_key_id = requiredValue("resource-id");
  }
  if (action === "create" || action === "update" || action === "issue") {
    request.body = parseResourceBody();
  }
  return request;
}

function requiredScopeId(label: string, value: string): string {
  if (value.length === 0) {
    throw new Error(`${label} is required for this operation`);
  }
  return value;
}

function advanceRevision(response: Response): void {
  const etag = response.headers.get("ETag");
  const token = etag?.match(/^"?(rev-[0-9]+)"?$/u)?.[1];
  if (token !== undefined) {
    element<HTMLInputElement>("config-revision").value = token;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function displayIssuedClientKey(value: unknown): unknown {
  if (!isRecord(value) || typeof value.key !== "string" || value.key.length === 0) {
    throw new Error("the management server did not return a valid one-time Client Key");
  }
  const { key, ...metadata } = value;
  element<HTMLElement>("issued-client-key").textContent = key;
  element<HTMLElement>("issued-client-key-panel").hidden = false;
  return metadata;
}

function redactUnexpectedClientKey(value: unknown): unknown {
  if (!isRecord(value) || !("key" in value)) {
    return value;
  }
  const { key: _key, ...metadata } = value;
  return { ...metadata, key: "[redacted]" };
}

async function showResponse(response: Response, issuedClientKey = false): Promise<void> {
  clearIssuedClientKey();
  advanceRevision(response);
  const body = await response.text();
  let parsed: unknown = null;
  if (body.length > 0) {
    try {
      parsed = JSON.parse(body) as unknown;
    } catch {
      parsed = { response: "Management server returned a non-JSON response" };
    }
  }
  const safeBody = issuedClientKey && response.ok ? displayIssuedClientKey(parsed) : redactUnexpectedClientKey(parsed);
  setResult({ ok: response.ok, status: response.status, etag: response.headers.get("ETag"), body: safeBody });
}

function lifecycleAction(): LifecycleAction {
  return element<HTMLSelectElement>("configuration-lifecycle-action").value as LifecycleAction;
}

function backupAction(): BackupAction {
  return element<HTMLSelectElement>("backup-action").value as BackupAction;
}

function clearBackupArtifactSelection(): void {
  element<HTMLInputElement>("backup-artifact").value = "";
}

function selectedBackupArtifact(): File {
  const artifact = element<HTMLInputElement>("backup-artifact").files?.item(0);
  if (artifact === null || artifact === undefined || artifact.size === 0) {
    throw new Error("select one non-empty encrypted backup artifact");
  }
  return artifact;
}

function updateBackupFields(): void {
  const needsArtifact = backupAction() !== "sourcePreflight";
  element<HTMLElement>("backup-artifact-field").hidden = !needsArtifact;
  element<HTMLInputElement>("backup-artifact").required = needsArtifact;
  if (!needsArtifact) clearBackupArtifactSelection();
}

function backupRequest(action: BackupAction): { operation: ManagementOperationName; request: ManagementRequest } {
  switch (action) {
    case "sourcePreflight":
      return { operation: "previewBackup", request: {} };
    case "restorePreflight":
      return { operation: "previewRestore", request: { body: selectedBackupArtifact() } };
    case "restore":
      return { operation: "restoreBackup", request: { body: selectedBackupArtifact() } };
  }
}

function updateLifecycleFields(): void {
  const action = lifecycleAction();
  const versionRequired = action === "get" || action === "create" || action === "validate" || action === "publish";
  const create = action === "create";
  element<HTMLElement>("configuration-version-field").hidden = !versionRequired;
  element<HTMLElement>("configuration-parent-field").hidden = !create;
  element<HTMLElement>("configuration-description-field").hidden = !create;
  element<HTMLInputElement>("configuration-version-id").required = versionRequired;
  element<HTMLTextAreaElement>("configuration-description").required = create;
}

function lifecycleRequest(action: LifecycleAction): { operation: ManagementOperationName; request: ManagementRequest } {
  const versionId = action === "get" || action === "create" || action === "validate" || action === "publish"
    ? requiredValue("configuration-version-id")
    : "";
  switch (action) {
    case "list":
      return { operation: "listConfigVersions", request: {} };
    case "get":
      return { operation: "getConfigVersion", request: { path: { config_version_id: versionId } } };
    case "create": {
      const parentId = optionalValue("configuration-parent-id");
      return {
        operation: "createConfigVersion",
        request: {
          body: {
            id: versionId,
            parent_id: parentId,
            description: requiredValue("configuration-description"),
          },
        },
      };
    }
    case "validate":
      return { operation: "validateConfigVersion", request: { path: { config_version_id: versionId } } };
    case "publish":
      return {
        operation: "publishConfigVersion",
        request: { path: { config_version_id: versionId }, headers: lifecycleHeaders(true) },
      };
    case "rollback":
      return { operation: "rollbackConfigVersion", request: { headers: lifecycleHeaders(true) } };
    case "audit":
      return { operation: "listManagementAuditEvents", request: {} };
  }
}

function api(): ManagementApi {
  if (managementApi === undefined || session === undefined) {
    throw new Error("connect the management client before running an operation");
  }
  return managementApi;
}

function populateResourceActions(): void {
  const select = element<HTMLSelectElement>("resource-action");
  const actions = resourceActions[currentResourceKind()];
  select.replaceChildren(
    ...actions.map((action) => {
      const option = document.createElement("option");
      option.value = action;
      option.textContent = action;
      return option;
    }),
  );
}

function populateResourceTemplate(): void {
  element<HTMLTextAreaElement>("resource-json").value = resourceTemplates[currentResourceKind()];
}

function updateResourceFields(): void {
  const kind = currentResourceKind();
  const action = currentResourceAction();
  const resourceIdNeeded = (
    (kind === "egress" || kind === "upstream" || kind === "publicModel" || kind === "accessGroup")
      && action !== "list" && action !== "create"
  ) || (kind === "endpoint" || kind === "credential") && action !== "create"
    || kind === "binding"
    || kind === "route" && action !== "create"
    || kind === "clientKey" && action !== "list" && action !== "issue";
  const scopeLabel: Partial<Record<ResourceKind, string>> = {
    endpoint: "Owning Upstream ID",
    credential: "Owning Upstream ID",
    modelAlias: "Public Model ID",
    route: "Public Model ID",
    candidate: "Route ID",
    accessGroupRoute: "Access Group ID",
  };
  const scopeNeeded = (kind === "endpoint" || kind === "credential" || kind === "route") && action === "create"
    || kind === "modelAlias" || kind === "candidate" || kind === "accessGroupRoute";
  const resourceLabels: Readonly<Record<ResourceKind, string>> = {
    egress: "Egress policy ID", upstream: "Upstream ID", endpoint: "Endpoint ID", credential: "Credential ID",
    binding: "Endpoint ID", publicModel: "Public Model ID", modelAlias: "Alias ID", route: "Route ID",
    candidate: "Candidate ID", accessGroup: "Access Group ID", accessGroupRoute: "Grant ID", clientKey: "Client Key ID",
  };
  const resourceInput = element<HTMLInputElement>("resource-id");
  const scopeInput = element<HTMLInputElement>("resource-scope-id");
  element<HTMLElement>("resource-id-label").textContent = resourceLabels[kind];
  element<HTMLElement>("resource-scope-label").textContent = scopeLabel[kind] ?? "Parent resource ID";
  element<HTMLElement>("resource-id-field").hidden = !resourceIdNeeded;
  element<HTMLElement>("resource-scope-field").hidden = !scopeNeeded;
  resourceInput.required = resourceIdNeeded;
  scopeInput.required = scopeNeeded;
  if (!resourceIdNeeded) resourceInput.value = "";
  if (!scopeNeeded) scopeInput.value = "";
}

function clearResourceIdentifiers(): void {
  element<HTMLInputElement>("resource-id").value = "";
  element<HTMLInputElement>("resource-scope-id").value = "";
}

function installHandlers(): void {
  const resourceKind = element<HTMLSelectElement>("resource-kind");
  resourceKind.addEventListener("change", () => {
    clearResourceIdentifiers();
    populateResourceActions();
    populateResourceTemplate();
    updateResourceFields();
  });
  element<HTMLSelectElement>("resource-action").addEventListener("change", updateResourceFields);
  populateResourceActions();
  populateResourceTemplate();
  updateResourceFields();
  element<HTMLSelectElement>("configuration-lifecycle-action").addEventListener("change", updateLifecycleFields);
  updateLifecycleFields();
  element<HTMLSelectElement>("backup-action").addEventListener("change", updateBackupFields);
  updateBackupFields();

  element<HTMLFormElement>("session-form").addEventListener("submit", (event) => {
    event.preventDefault();
    try {
      clearIssuedClientKey();
      const managementKey = requiredValue("management-key");
      const csrfToken = requiredValue("csrf-token");
      session = { managementKey, csrfToken };
      managementApi = new ManagementApi({
        managementKey: () => session?.managementKey,
        csrfToken: () => session?.csrfToken,
      });
      element<HTMLElement>("connection-status").textContent = "Connected in memory";
      setResult({ ok: true, message: "Management client connected for this page lifetime" });
    } catch (error) {
      setFailure(error instanceof Error ? error.message : "management session setup failed");
    }
  });

  element<HTMLFormElement>("resource-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    try {
      const kind = currentResourceKind();
      const action = currentResourceAction();
      await showResponse(
        await api().request(operationForResource(kind, action), resourceRequest(kind, action)),
        kind === "clientKey" && action === "issue",
      );
    } catch (error) {
      setFailure(error instanceof Error ? error.message : "management resource operation failed");
    }
  });

  element<HTMLFormElement>("endpoint-workflow-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    try {
      const endpointId = requiredValue("workflow-endpoint-id");
      const catalogAction = element<HTMLSelectElement>("catalog-action").value;
      if (catalogAction === "preview") {
        await showResponse(await api().request("previewCatalogDiscovery", { path: { endpoint_id: endpointId }, headers: headers(false) }));
        return;
      }
      if (catalogAction === "apply") {
        await showResponse(await api().request("applyCatalogDiscovery", { path: { endpoint_id: endpointId }, headers: headers(true) }));
        return;
      }
      const mode = element<HTMLSelectElement>("endpoint-test-mode").value;
      await showResponse(await api().request("testEndpoint", { path: { endpoint_id: endpointId }, headers: headers(false), body: { mode } }));
    } catch (error) {
      setFailure(error instanceof Error ? error.message : "endpoint workflow failed");
    }
  });

  element<HTMLFormElement>("oauth-workflow-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    try {
      const credentialId = requiredValue("workflow-credential-id");
      const action = element<HTMLSelectElement>("oauth-action").value;
      const operation: ManagementOperationName = action === "start"
        ? "startCredentialOAuth"
        : action === "status"
          ? "getCredentialOAuthStatus"
          : "cancelCredentialOAuth";
      await showResponse(await api().request(operation, { path: { credential_id: credentialId }, headers: headers(false) }));
    } catch (error) {
      setFailure(error instanceof Error ? error.message : "OAuth workflow failed");
    }
  });

  element<HTMLButtonElement>("read-catalog-status").addEventListener("click", async () => {
    try {
      await showResponse(await api().request("getCatalogStatus", { headers: headers(false) }));
    } catch (error) {
      setFailure(error instanceof Error ? error.message : "Catalog status request failed");
    }
  });

  element<HTMLButtonElement>("read-runtime-availability").addEventListener("click", async () => {
    try {
      await showResponse(await api().request("getRuntimeAvailability", { headers: headers(false) }));
    } catch (error) {
      setFailure(error instanceof Error ? error.message : "runtime availability request failed");
    }
  });

  element<HTMLFormElement>("quota-recovery-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    try {
      await showResponse(await api().request("requestQuotaRecovery", {
        headers: headers(false),
        body: {
          endpoint_id: requiredValue("runtime-endpoint-id"),
          credential_id: requiredValue("runtime-credential-id"),
          upstream_model: optionalValue("runtime-upstream-model"),
        },
      }));
    } catch (error) {
      setFailure(error instanceof Error ? error.message : "quota recovery request failed");
    }
  });

  element<HTMLFormElement>("route-explain-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    try {
      await showResponse(await api().request("explainRoute", {
        path: { route_id: requiredValue("runtime-route-id") },
        query: {
          requested_model: requiredValue("runtime-requested-model"),
          protocol: element<HTMLSelectElement>("runtime-protocol").value,
        },
        headers: headers(false),
      }));
    } catch (error) {
      setFailure(error instanceof Error ? error.message : "Route Explain request failed");
    }
  });

  element<HTMLFormElement>("request-attempts-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    try {
      await showResponse(await api().request("listRequestAttempts", {
        path: { request_id: requiredValue("runtime-request-id") },
      }));
    } catch (error) {
      setFailure(error instanceof Error ? error.message : "request attempt lookup failed");
    }
  });

  element<HTMLFormElement>("configuration-lifecycle-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    try {
      const { operation, request } = lifecycleRequest(lifecycleAction());
      await showResponse(await api().request(operation, request));
    } catch (error) {
      setFailure(error instanceof Error ? error.message : "configuration lifecycle operation failed");
    }
  });

  element<HTMLFormElement>("backup-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    try {
      const { operation, request } = backupRequest(backupAction());
      await showResponse(await api().request(operation, request));
    } catch (error) {
      setFailure(error instanceof Error ? error.message : "backup operation failed");
    } finally {
      clearBackupArtifactSelection();
    }
  });

  element<HTMLButtonElement>("clear-issued-client-key").addEventListener("click", () => {
    clearIssuedClientKey();
    setResult({ ok: true, message: "One-time Client Key cleared from this page" });
  });
}

function mount(): void {
  const root = document.querySelector<HTMLElement>("#app");
  if (root === null) {
    throw new Error("management application root is missing");
  }
  root.innerHTML = applicationMarkup();
  installHandlers();
}

mount();
