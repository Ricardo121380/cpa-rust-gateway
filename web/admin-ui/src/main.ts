/**
 * P10-04 protected Upstream workspace.
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

type ResourceKind = "egress" | "upstream" | "endpoint" | "credential" | "binding";
type ResourceAction = "list" | "get" | "create" | "update" | "delete";

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
};

function applicationMarkup(): string {
  return `
    <div class="shell">
      <aside class="sidebar">
        <p class="brand">CPA Rust Gateway<span>Management plane</span></p>
        <nav class="navigation" aria-label="Management sections">
          <a aria-current="page" href="#upstreams">Upstreams</a>
          <a aria-disabled="true" href="#routing">Routing</a>
          <a aria-disabled="true" href="#runtime">Runtime</a>
          <a aria-disabled="true" href="#configuration">Configuration</a>
        </nav>
        <p class="sidebar-note">P10-04 only. Later workspaces remain unavailable.</p>
      </aside>
      <main class="content" id="upstreams">
        <header>
          <p class="eyebrow">P10-04 · protected workspace</p>
          <h1>Upstreams and credentials</h1>
          <p class="lead">Manage draft egress policies, upstreams, endpoints, credentials and bindings. Endpoint tests and OAuth actions use only the server-side admitted workflow.</p>
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

        <section class="panel" aria-labelledby="resource-heading">
          <div class="panel-heading"><div><p class="eyebrow">Draft resources</p><h2 id="resource-heading">CRUD and binding controls</h2></div></div>
          <form id="resource-form" class="form-grid" novalidate>
            <label>Resource<select id="resource-kind" name="resource-kind">
              <option value="egress">Egress policy</option><option value="upstream">Upstream</option>
              <option value="endpoint">Endpoint</option><option value="credential">Credential</option>
              <option value="binding">Endpoint credential binding</option>
            </select></label>
            <label>Action<select id="resource-action" name="resource-action"></select></label>
            <label>Resource ID<input id="resource-id" name="resource-id" autocomplete="off"></label>
            <label>Owning Upstream ID<input id="upstream-id" name="upstream-id" autocomplete="off"></label>
            <label class="wide">Resource JSON<textarea id="resource-json" name="resource-json" rows="13" spellcheck="false"></textarea></label>
            <div class="form-actions"><button type="submit">Run protected operation</button></div>
          </form>
          <p class="muted">Create or update Credential JSON accepts a Secret once for immediate server-side sealing. Successful responses contain metadata only.</p>
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

        <section class="panel" aria-labelledby="result-heading">
          <div class="panel-heading"><div><p class="eyebrow">Safe result</p><h2 id="result-heading">Operation response</h2></div></div>
          <pre id="operation-result" class="result" aria-live="polite">No operation has run.</pre>
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

function setFailure(message: string): void {
  setResult({ ok: false, message });
}

function requiredValue(id: string): string {
  const value = element<HTMLInputElement>(id).value.trim();
  if (value.length === 0) {
    throw new Error("a required management input is missing");
  }
  return value;
}

function headers(includeRevision: boolean): Record<string, string> {
  const values: Record<string, string> = { "X-Config-Version": requiredValue("config-version") };
  if (includeRevision) {
    values["If-Match"] = requiredValue("config-revision");
  }
  return values;
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
  };
  const operation = operations[kind][action];
  if (operation === undefined) {
    throw new Error("that resource action is not available");
  }
  return operation;
}

function resourceRequest(kind: ResourceKind, action: ResourceAction): ManagementRequest {
  const upstreamId = element<HTMLInputElement>("upstream-id").value.trim();
  const mutates = action === "create" || action === "update" || action === "delete";
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
      if (upstreamId.length === 0) {
        throw new Error("an owning Upstream ID is required to create an Endpoint");
      }
      request.path.upstream_id = upstreamId;
    } else {
      request.path.endpoint_id = requiredValue("resource-id");
    }
  }
  if (kind === "credential") {
    if (action === "create") {
      if (upstreamId.length === 0) {
        throw new Error("an owning Upstream ID is required to create a Credential");
      }
      request.path.upstream_id = upstreamId;
    } else {
      request.path.credential_id = requiredValue("resource-id");
    }
  }
  if (kind === "binding") {
    request.path.endpoint_id = requiredValue("resource-id");
  }
  if (action === "create" || action === "update") {
    request.body = parseResourceBody();
  }
  return request;
}

function advanceRevision(response: Response): void {
  const etag = response.headers.get("ETag");
  const token = etag?.match(/^"?(rev-[0-9]+)"?$/u)?.[1];
  if (token !== undefined) {
    element<HTMLInputElement>("config-revision").value = token;
  }
}

async function showResponse(response: Response): Promise<void> {
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
  setResult({ ok: response.ok, status: response.status, etag: response.headers.get("ETag"), body: parsed });
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

function installHandlers(): void {
  const resourceKind = element<HTMLSelectElement>("resource-kind");
  resourceKind.addEventListener("change", () => {
    populateResourceActions();
    populateResourceTemplate();
  });
  populateResourceActions();
  populateResourceTemplate();

  element<HTMLFormElement>("session-form").addEventListener("submit", (event) => {
    event.preventDefault();
    try {
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
      await showResponse(await api().request(operationForResource(kind, action), resourceRequest(kind, action)));
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
