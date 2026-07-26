// Dev-only fixture backend, injected through ManagementClientOptions.fetch —
// the sanctioned seam that keeps "generated client is the only fetch path"
// (C5) intact. Never bundled in production: the import site is guarded by
// import.meta.env.DEV, so release builds drop this module entirely.
//
// Behavior mirrors the real contract closely enough to exercise the revision
// pipeline: version-scoped reads return ETag "rev-N"; mutations demand a
// matching If-Match and advance the revision; mismatches produce 409 with the
// uniform error envelope.

type VersionRow = {
  id: string;
  parent_id: string | null;
  status: "draft" | "active" | "archived";
  revision: number;
  created_at_ms: number;
  description: string;
};

type GroupRow = {
  id: string;
  name: string;
  status: "active" | "disabled";
  limits: Record<string, number>;
};

type KeyRow = {
  id: string;
  access_group_id: string;
  prefix: string;
  status: "active" | "disabled" | "revoked";
  expires_at_ms: number | null;
};

type EgressRow = {
  id: string;
  name: string;
  allowed_schemes: string[];
  allowed_hosts: string[];
  allowed_ports: number[];
  allowed_cidrs: string[];
  redirect_mode: "deny" | "revalidate";
  max_redirects: number;
};

type UpstreamRow = {
  id: string;
  name: string;
  kind: string;
  enabled: boolean;
  tags: string[];
  egress_policy_id: string | null;
};

type ModelRow = {
  id: string;
  model_name: string;
  status: "active" | "disabled";
  display_name: string;
  capabilities: Record<string, boolean>;
};

type EndpointRow = {
  id: string;
  upstream_id: string;
  adapter_id: string;
  api_format: string;
  base_url: string;
  inference_path: string;
  models_path: string | null;
  transport: "https";
  enabled: boolean;
};

type CredentialRow = {
  id: string;
  upstream_id: string;
  kind: string;
  status: "active" | "disabled" | "revoked";
  revision: number;
  secret_present: true;
};

type BindingRow = {
  endpoint_id: string;
  upstream_id: string;
  credential_id: string;
  enabled: boolean;
  priority: number;
  weight: number;
  concurrency: number;
};

type OAuthOp = { state: "pending" | "complete" | "cancelled" | "failed"; polls: number; expires_at_ms: number };

type AuditRow = {
  id: number;
  action: string;
  actor: string;
  occurred_at_ms: number;
  config_version_id: string;
  replaced_config_version_id: string | null;
};

const state = {
  versions: [
    {
      id: "v-2026-07",
      parent_id: null,
      status: "active",
      revision: 87,
      created_at_ms: 1784000000000,
      description: "两站 minimax-m3 聚合(演示数据)",
    },
    {
      id: "draft-2026-08",
      parent_id: "v-2026-07",
      status: "draft",
      revision: 4,
      created_at_ms: 1785000000000,
      description: "新增 grok.build 凭据池(演示数据)",
    },
  ] as VersionRow[],
  groups: new Map<string, GroupRow[]>([
    [
      "draft-2026-08",
      [
        { id: "team-default", name: "默认组", status: "active", limits: { max_concurrency: 4 } },
        { id: "team-batch", name: "批处理组", status: "active", limits: { max_concurrency: 16 } },
      ],
    ],
  ]),
  keys: new Map<string, KeyRow[]>([
    [
      "draft-2026-08",
      [
        {
          id: "key-cli",
          access_group_id: "team-default",
          prefix: "rgw_9f3c21ab04d7e6b2",
          status: "active",
          expires_at_ms: null,
        },
      ],
    ],
  ]),
  keyCounter: 1,
  egress: new Map<string, EgressRow[]>([
    [
      "draft-2026-08",
      [
        {
          id: "relay-only",
          name: "仅中转站",
          allowed_schemes: ["https"],
          allowed_hosts: ["relay-a.example.com", "relay-b.example.com"],
          allowed_ports: [443],
          allowed_cidrs: [],
          redirect_mode: "deny",
          max_redirects: 0,
        },
      ],
    ],
  ]),
  upstreams: new Map<string, UpstreamRow[]>([
    [
      "draft-2026-08",
      [
        {
          id: "relay-a",
          name: "中转站 A",
          kind: "openai-compatible",
          enabled: true,
          tags: ["minimax-m3"],
          egress_policy_id: "relay-only",
        },
        {
          id: "grok-build-pool",
          name: "Grok Build 池",
          kind: "grok.build",
          enabled: false,
          tags: [],
          egress_policy_id: null,
        },
      ],
    ],
  ]),
  endpoints: new Map<string, EndpointRow[]>([
    [
      "draft-2026-08",
      [
        {
          id: "ep-relay-a-responses",
          upstream_id: "relay-a",
          adapter_id: "openai-compatible",
          api_format: "openai/responses",
          base_url: "https://relay-a.example.com/v1",
          inference_path: "/responses",
          models_path: "/models",
          transport: "https",
          enabled: true,
        },
        {
          id: "ep-grok-build",
          upstream_id: "grok-build-pool",
          adapter_id: "grok.build",
          api_format: "openai/responses",
          base_url: "https://cli-chat-proxy.grok.com/v1",
          inference_path: "/responses",
          models_path: null,
          transport: "https",
          enabled: false,
        },
      ],
    ],
  ]),
  credentials: new Map<string, CredentialRow[]>([
    [
      "draft-2026-08",
      [
        { id: "cred-relay-key", upstream_id: "relay-a", kind: "api_key", status: "active", revision: 2, secret_present: true },
        { id: "cred-grok-oauth", upstream_id: "grok-build-pool", kind: "oauth", status: "disabled", revision: 0, secret_present: true },
      ],
    ],
  ]),
  bindings: new Map<string, BindingRow[]>([
    [
      "draft-2026-08",
      [
        {
          endpoint_id: "ep-relay-a-responses",
          upstream_id: "relay-a",
          credential_id: "cred-relay-key",
          enabled: true,
          priority: 0,
          weight: 1,
          concurrency: 4,
        },
      ],
    ],
  ]),
  oauthOps: new Map<string, OAuthOp>(),
  models: new Map<string, ModelRow[]>([
    [
      "draft-2026-08",
      [
        {
          id: "pm-minimax",
          model_name: "minimax-m3",
          status: "active",
          display_name: "MiniMax M3(聚合)",
          capabilities: { streaming: true, tools: true, reasoning: true },
        },
      ],
    ],
  ]),
  aliases: new Map<string, { alias: string; public_model_id: string }[]>(),
  routes: new Map<string, { id: string; public_model_id: string }[]>(),
  audit: [
    {
      id: 1,
      action: "config_created",
      actor: "management-key",
      occurred_at_ms: 1785000000000,
      config_version_id: "draft-2026-08",
      replaced_config_version_id: null,
    },
    {
      id: 2,
      action: "config_published",
      actor: "management-key",
      occurred_at_ms: 1784100000000,
      config_version_id: "v-2026-07",
      replaced_config_version_id: null,
    },
  ] as AuditRow[],
};

function revisionToken(version: VersionRow): string {
  return `rev-${version.revision}`;
}

function json(status: number, body: unknown, etag?: string): Response {
  const headers = new Headers({ "Content-Type": "application/json" });
  if (etag !== undefined) {
    headers.set("ETag", `"${etag}"`);
  }
  return new Response(JSON.stringify(body), { status, headers });
}

function errorResponse(status: number, code: string, message: string): Response {
  return json(status, { error: { code, message } });
}

function versionByHeader(headers: Headers): VersionRow | Response {
  const id = headers.get("X-Config-Version");
  const found = state.versions.find((version) => version.id === id);
  if (found === undefined) {
    return errorResponse(409, "management_lifecycle_conflict", "unknown config version");
  }
  return found;
}

function requireDraftAndMatch(version: VersionRow, headers: Headers): Response | undefined {
  if (version.status !== "draft") {
    return errorResponse(409, "management_lifecycle_conflict", "mutation requires a draft version");
  }
  const ifMatch = headers.get("If-Match")?.replace(/"/gu, "");
  if (ifMatch !== revisionToken(version)) {
    return errorResponse(409, "management_revision_conflict", "revision token is stale");
  }
  return undefined;
}

function hex(length: number): string {
  let out = "";
  for (let index = 0; index < length; index += 1) {
    out += "0123456789abcdef"[(index * 7 + state.keyCounter * 13) % 16];
  }
  return out;
}

export const fixtureFetch: typeof fetch = (input, init) => {
  // No `new Request(...)`: Node's Request rejects relative URLs, and the
  // generated client always issues relative /admin paths.
  const raw =
    typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url;
  const url = new URL(raw, "http://prism.fixture");
  const method = init?.method ?? "GET";
  const headers = new Headers(init?.headers);
  const bodyText = typeof init?.body === "string" ? init.body : undefined;
  const route = `${method} ${url.pathname}`;

  const respond = async (): Promise<Response> => {
    if (headers.get("X-Management-Key") === null) {
      return errorResponse(404, "management_access_denied", "Management access is unavailable");
    }

    if (route === "GET /admin/config-versions") {
      return json(
        200,
        state.versions.map((version) => ({ ...version, revision: revisionToken(version) })),
      );
    }

    if (route === "POST /admin/config-versions") {
      const body = JSON.parse(bodyText ?? "{}") as { id: string; parent_id?: string | null; description: string };
      if (state.versions.some((version) => version.id === body.id)) {
        return errorResponse(409, "management_lifecycle_conflict", "config version id already exists");
      }
      const row: VersionRow = {
        id: body.id,
        parent_id: body.parent_id ?? null,
        status: "draft",
        revision: 0,
        created_at_ms: 1785100000000,
        description: body.description,
      };
      state.versions.push(row);
      state.groups.set(row.id, []);
      state.keys.set(row.id, []);
      return json(201, { ...row, revision: revisionToken(row) });
    }

    const validate = /^POST \/admin\/config-versions\/([^/]+)\/validate$/u.exec(route);
    if (validate !== null) {
      const version = state.versions.find((row) => row.id === decodeURIComponent(validate[1] ?? ""));
      if (version === undefined) {
        return errorResponse(409, "management_lifecycle_conflict", "unknown config version");
      }
      const empty = (state.keys.get(version.id) ?? []).length === 0 && (state.groups.get(version.id) ?? []).length === 0;
      return json(200, empty ? { valid: false, error_codes: ["route_missing_active_candidate"] } : { valid: true, error_codes: [] });
    }

    const publish = /^POST \/admin\/config-versions\/([^/]+)\/publish$/u.exec(route);
    if (publish !== null) {
      const version = state.versions.find((row) => row.id === decodeURIComponent(publish[1] ?? ""));
      if (version === undefined || version.status !== "draft") {
        return errorResponse(409, "management_lifecycle_conflict", "publish requires an existing draft");
      }
      const ifMatch = headers.get("If-Match")?.replace(/"/gu, "");
      if (ifMatch !== revisionToken(version)) {
        return errorResponse(409, "management_revision_conflict", "revision token is stale");
      }
      const replaced = state.versions.find((row) => row.status === "active");
      if (replaced !== undefined) {
        replaced.status = "archived";
      }
      version.status = "active";
      return json(200, {
        active_config_version_id: version.id,
        replaced_config_version_id: replaced?.id ?? null,
      });
    }

    if (route === "POST /admin/config-versions/rollback") {
      const active = state.versions.find((row) => row.status === "active");
      const predecessor = state.versions.find((row) => row.status === "archived" && row.id === active?.parent_id)
        ?? state.versions.find((row) => row.status === "archived");
      if (active === undefined || predecessor === undefined) {
        return errorResponse(409, "management_lifecycle_conflict", "no persisted rollback target");
      }
      active.status = "archived";
      predecessor.status = "active";
      return json(200, {
        active_config_version_id: predecessor.id,
        replaced_config_version_id: active.id,
      });
    }

    if (route === "GET /admin/access-groups") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      return json(200, state.groups.get(version.id) ?? [], revisionToken(version));
    }

    if (route === "GET /admin/client-keys") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      return json(200, state.keys.get(version.id) ?? [], revisionToken(version));
    }

    if (route === "POST /admin/client-keys") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const rejected = requireDraftAndMatch(version, headers);
      if (rejected !== undefined) return rejected;
      const body = JSON.parse(bodyText ?? "{}") as {
        id: string;
        access_group_id: string;
        status: "active" | "disabled" | "revoked";
        expires_at_ms?: number | null;
      };
      state.keyCounter += 1;
      const row: KeyRow = {
        id: body.id,
        access_group_id: body.access_group_id,
        prefix: `rgw_${hex(16)}`,
        status: body.status,
        expires_at_ms: body.expires_at_ms ?? null,
      };
      (state.keys.get(version.id) ?? []).push(row);
      version.revision += 1;
      return json(201, { ...row, key: `${row.prefix}_${hex(64)}` }, revisionToken(version));
    }

    const revoke = /^DELETE \/admin\/client-keys\/([^/]+)$/u.exec(route);
    if (revoke !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const rejected = requireDraftAndMatch(version, headers);
      if (rejected !== undefined) return rejected;
      const row = (state.keys.get(version.id) ?? []).find(
        (key) => key.id === decodeURIComponent(revoke[1] ?? ""),
      );
      if (row === undefined) {
        return errorResponse(409, "management_lifecycle_conflict", "unknown client key");
      }
      row.status = "revoked";
      version.revision += 1;
      const headersOut = new Headers({ ETag: `"${revisionToken(version)}"` });
      return new Response(null, { status: 204, headers: headersOut });
    }

    // ---- egress policies ----
    if (route === "GET /admin/egress-policies") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      return json(200, state.egress.get(version.id) ?? [], revisionToken(version));
    }
    if (route === "POST /admin/egress-policies") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const rejected = requireDraftAndMatch(version, headers);
      if (rejected !== undefined) return rejected;
      const row = JSON.parse(bodyText ?? "{}") as EgressRow;
      (state.egress.get(version.id) ?? state.egress.set(version.id, []).get(version.id))?.push(row);
      version.revision += 1;
      return json(201, row, revisionToken(version));
    }
    const egressItem = /^(PATCH|DELETE) \/admin\/egress-policies\/([^/]+)$/u.exec(route);
    if (egressItem !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const rejected = requireDraftAndMatch(version, headers);
      if (rejected !== undefined) return rejected;
      const list = state.egress.get(version.id) ?? [];
      const id = decodeURIComponent(egressItem[2] ?? "");
      const index = list.findIndex((row) => row.id === id);
      if (index === -1) {
        return errorResponse(409, "management_lifecycle_conflict", "unknown egress policy");
      }
      version.revision += 1;
      if (egressItem[1] === "DELETE") {
        list.splice(index, 1);
        for (const upstream of state.upstreams.get(version.id) ?? []) {
          if (upstream.egress_policy_id === id) upstream.egress_policy_id = null;
        }
        return new Response(null, {
          status: 204,
          headers: new Headers({ ETag: `"${revisionToken(version)}"` }),
        });
      }
      const next = { ...JSON.parse(bodyText ?? "{}"), id } as EgressRow;
      list[index] = next;
      return json(200, next, revisionToken(version));
    }

    // ---- upstreams (top level) ----
    if (route === "GET /admin/upstreams") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      return json(200, state.upstreams.get(version.id) ?? [], revisionToken(version));
    }
    if (route === "POST /admin/upstreams") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const rejected = requireDraftAndMatch(version, headers);
      if (rejected !== undefined) return rejected;
      const row = JSON.parse(bodyText ?? "{}") as UpstreamRow;
      (state.upstreams.get(version.id) ?? state.upstreams.set(version.id, []).get(version.id))?.push(row);
      version.revision += 1;
      return json(201, row, revisionToken(version));
    }
    const upstreamItem = /^(PATCH|DELETE) \/admin\/upstreams\/([^/]+)$/u.exec(route);
    if (upstreamItem !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const rejected = requireDraftAndMatch(version, headers);
      if (rejected !== undefined) return rejected;
      const list = state.upstreams.get(version.id) ?? [];
      const id = decodeURIComponent(upstreamItem[2] ?? "");
      const index = list.findIndex((row) => row.id === id);
      if (index === -1) {
        return errorResponse(409, "management_lifecycle_conflict", "unknown upstream");
      }
      version.revision += 1;
      if (upstreamItem[1] === "DELETE") {
        list.splice(index, 1);
        return new Response(null, {
          status: 204,
          headers: new Headers({ ETag: `"${revisionToken(version)}"` }),
        });
      }
      const next = { ...JSON.parse(bodyText ?? "{}"), id } as UpstreamRow;
      list[index] = next;
      return json(200, next, revisionToken(version));
    }

    // ---- public models (+ insert-only aliases / routes) ----
    if (route === "GET /admin/public-models") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      return json(200, state.models.get(version.id) ?? [], revisionToken(version));
    }
    if (route === "POST /admin/public-models") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const rejected = requireDraftAndMatch(version, headers);
      if (rejected !== undefined) return rejected;
      const row = JSON.parse(bodyText ?? "{}") as ModelRow;
      (state.models.get(version.id) ?? state.models.set(version.id, []).get(version.id))?.push(row);
      version.revision += 1;
      return json(201, row, revisionToken(version));
    }
    const modelItem = /^(PATCH|DELETE) \/admin\/public-models\/([^/]+)$/u.exec(route);
    if (modelItem !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const rejected = requireDraftAndMatch(version, headers);
      if (rejected !== undefined) return rejected;
      const list = state.models.get(version.id) ?? [];
      const id = decodeURIComponent(modelItem[2] ?? "");
      const index = list.findIndex((row) => row.id === id);
      if (index === -1) {
        return errorResponse(409, "management_lifecycle_conflict", "unknown public model");
      }
      version.revision += 1;
      if (modelItem[1] === "DELETE") {
        list.splice(index, 1);
        return new Response(null, {
          status: 204,
          headers: new Headers({ ETag: `"${revisionToken(version)}"` }),
        });
      }
      const next = { ...JSON.parse(bodyText ?? "{}"), id } as ModelRow;
      list[index] = next;
      return json(200, next, revisionToken(version));
    }
    const aliasCreate = /^POST \/admin\/public-models\/([^/]+)\/aliases$/u.exec(route);
    if (aliasCreate !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const rejected = requireDraftAndMatch(version, headers);
      if (rejected !== undefined) return rejected;
      const modelId = decodeURIComponent(aliasCreate[1] ?? "");
      const body = JSON.parse(bodyText ?? "{}") as { alias: string };
      const models = state.models.get(version.id) ?? [];
      if (models.some((row) => row.model_name === body.alias)) {
        return errorResponse(409, "management_lifecycle_conflict", "alias conflicts with an active model name");
      }
      (state.aliases.get(version.id) ?? state.aliases.set(version.id, []).get(version.id))?.push({
        alias: body.alias,
        public_model_id: modelId,
      });
      version.revision += 1;
      return json(201, { alias: body.alias, public_model_id: modelId }, revisionToken(version));
    }
    const routeCreate = /^POST \/admin\/public-models\/([^/]+)\/routes$/u.exec(route);
    if (routeCreate !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const rejected = requireDraftAndMatch(version, headers);
      if (rejected !== undefined) return rejected;
      const modelId = decodeURIComponent(routeCreate[1] ?? "");
      const routes = state.routes.get(version.id) ?? state.routes.set(version.id, []).get(version.id) ?? [];
      if (routes.some((row) => row.public_model_id === modelId)) {
        return errorResponse(409, "management_lifecycle_conflict", "model already has a route (1:1)");
      }
      const body = JSON.parse(bodyText ?? "{}") as { id: string };
      routes.push({ id: body.id, public_model_id: modelId });
      version.revision += 1;
      return json(201, { ...JSON.parse(bodyText ?? "{}"), public_model_id: modelId }, revisionToken(version));
    }

    // ---- PROPOSED G1: full graph (shape per CR-FE-001-shapes doc) ----
    const graph = /^GET \/admin\/config-versions\/([^/]+)\/graph$/u.exec(route);
    if (graph !== null) {
      const version = state.versions.find((row) => row.id === decodeURIComponent(graph[1] ?? ""));
      if (version === undefined) {
        return errorResponse(409, "management_lifecycle_conflict", "unknown config version");
      }
      return json(
        200,
        {
          config_version: { ...version, revision: revisionToken(version) },
          egress_policies: state.egress.get(version.id) ?? [],
          upstreams: state.upstreams.get(version.id) ?? [],
          endpoints: state.endpoints.get(version.id) ?? [],
          credentials: state.credentials.get(version.id) ?? [],
          bindings: state.bindings.get(version.id) ?? [],
          public_models: state.models.get(version.id) ?? [],
          aliases: state.aliases.get(version.id) ?? [],
          routes: state.routes.get(version.id) ?? [],
          candidates: [],
          access_groups: state.groups.get(version.id) ?? [],
          access_group_routes: [],
          client_keys: state.keys.get(version.id) ?? [],
        },
        revisionToken(version),
      );
    }

    // ---- credential OAuth (real contract ops; device-flow state machine) ----
    const oauthStart = /^POST \/admin\/credentials\/([^/]+)\/oauth\/start$/u.exec(route);
    if (oauthStart !== null) {
      const id = decodeURIComponent(oauthStart[1] ?? "");
      state.oauthOps.set(id, { state: "pending", polls: 0, expires_at_ms: Date.now() + 300_000 });
      const op = state.oauthOps.get(id) as OAuthOp;
      return json(202, { credential_id: id, state: op.state, expires_at_ms: op.expires_at_ms });
    }
    const oauthStatus = /^GET \/admin\/credentials\/([^/]+)\/oauth\/status$/u.exec(route);
    if (oauthStatus !== null) {
      const id = decodeURIComponent(oauthStatus[1] ?? "");
      const op = state.oauthOps.get(id);
      if (op === undefined) {
        return errorResponse(409, "management_lifecycle_conflict", "no oauth operation for credential");
      }
      if (op.state === "pending") {
        op.polls += 1;
        if (op.polls >= 3) {
          op.state = "complete"; // third poll succeeds — exercises the full pending path
        }
      }
      return json(200, { credential_id: id, state: op.state, expires_at_ms: op.expires_at_ms });
    }
    const oauthCancel = /^POST \/admin\/credentials\/([^/]+)\/oauth\/cancel$/u.exec(route);
    if (oauthCancel !== null) {
      const id = decodeURIComponent(oauthCancel[1] ?? "");
      const op = state.oauthOps.get(id);
      if (op !== undefined && op.state === "pending") {
        op.state = "cancelled";
      }
      return new Response(null, { status: 204 });
    }

    // ---- endpoint test + catalog discovery (real contract ops) ----
    const epTest = /^POST \/admin\/endpoints\/([^/]+)\/test$/u.exec(route);
    if (epTest !== null) {
      const id = decodeURIComponent(epTest[1] ?? "");
      const body = JSON.parse(bodyText ?? "{}") as { mode?: string };
      if (id === "ep-grok-build") {
        return json(200, { outcome: "transport_failed" });
      }
      return json(200, {
        outcome: "pass",
        status_class: "2xx",
        canonical_lifecycle: body.mode === "sse",
      });
    }
    const discoverPreview = /^POST \/admin\/endpoints\/([^/]+)\/models\/discover-preview$/u.exec(route);
    if (discoverPreview !== null) {
      return json(200, { added: 2, removed: 0, unchanged: 3 });
    }
    const discoverApply = /^POST \/admin\/endpoints\/([^/]+)\/models\/discover-apply$/u.exec(route);
    if (discoverApply !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const rejected = requireDraftAndMatch(version, headers);
      if (rejected !== undefined) return rejected;
      version.revision += 1;
      return json(200, { added: 2, removed: 0, unchanged: 3 }, revisionToken(version));
    }

    // ---- audit + backup ----
    if (route === "GET /admin/audit-events") {
      return json(200, state.audit);
    }
    if (route === "POST /admin/backups/preflight") {
      return json(200, { schema_version: 9, secret_key_required: true });
    }

    return errorResponse(503, "management_lifecycle_unavailable", `fixture: unhandled ${route}`);
  };

  return respond();
};
