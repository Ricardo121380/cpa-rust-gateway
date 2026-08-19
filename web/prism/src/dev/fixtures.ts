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

type OAuthOp = {
  state: "pending" | "complete" | "cancelled" | "failed" | "expired";
  polls: number;
  expires_at_ms: number;
  // The state token the fixture "issued" with the authorization URL. The
  // callback has to carry it back — that binding is the whole point of the
  // parameter, so the fixture checks it instead of accepting any paste.
  authState: string;
  authorization_url: string;
  failure_class?: string;
};

type GroupRouteRow = {
  access_group_id: string;
  route_id: string;
  enabled: boolean;
};

type RouteRow = {
  id: string;
  public_model_id: string;
  policy: "smooth_weighted_round_robin";
  max_attempts: number;
  bootstrap_timeout_ms: number;
};

type CandidateRow = {
  id: string;
  route_id: string;
  endpoint_id: string;
  upstream_model: string;
  credential_scope: "all_active";
  transform_mode: string;
  enabled: boolean;
  priority: number;
  weight: number;
  capability_override: Record<string, boolean>;
};

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
  // keyed "<configVersionId>:<accessGroupId>"
  groupRoutes: new Map<string, GroupRouteRow[]>([
    ["draft-2026-08:team-default", [{ access_group_id: "team-default", route_id: "rt-minimax", enabled: true }]],
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
        // Created but nothing bound yet — the common half-configured state,
        // and the one that proves the operational inventory is binding-driven
        // rather than upstream-driven.
        {
          id: "kiro-sub",
          name: "Kiro 订阅池",
          kind: "kiro",
          enabled: true,
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
        { id: "cred-grok-oauth", upstream_id: "grok-build-pool", kind: "oauth", status: "active", revision: 0, secret_present: true },
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
        // The oauth account needs a binding to exist at all: the operational
        // inventory is binding-driven, so an unbound credential is invisible
        // to the subresource panel — which is exactly the coverage boundary
        // the panel now states.
        {
          endpoint_id: "ep-grok-build",
          upstream_id: "grok-build-pool",
          credential_id: "cred-grok-oauth",
          enabled: false,
          priority: 1,
          weight: 1,
          concurrency: 2,
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
  routes: new Map<string, RouteRow[]>(),
  routeCandidates: new Map<string, CandidateRow[]>(),
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

/** Deterministic pseudo-noise in [0,1) — keeps demo data stable across reloads. */
function noise(index: number): number {
  const x = Math.sin(index * 12.9898) * 43758.5453;
  return x - Math.floor(x);
}

/** Stable small integer from an identifier, so a fixture series keyed on
 *  "cred-relay-key" looks the same on every refresh. */
function hashString(value: string): number {
  let hash = 0;
  for (let index = 0; index < value.length; index += 1) {
    hash = (hash * 31 + value.charCodeAt(index)) % 100_000;
  }
  return hash;
}

function hex(length: number): string {
  let out = "";
  for (let index = 0; index < length; index += 1) {
    out += "0123456789abcdef"[(index * 7 + state.keyCounter * 13) % 16];
  }
  return out;
}

// Prometheus exposition, matching telemetry.rs::render_prometheus exactly.
// Counters grow with each scrape so the overview's "since you opened this
// page" delta has something to show. The pipeline is rendered healthy on the
// Required path with diagnostics shedding — the common real state, and the one
// that proves the UI does not cry wolf over by-design backpressure.
let scrapes = 0;

function renderMetrics(scrape: number): string {
  const requests = 1180 + scrape * 7;
  const attempts = 1246 + scrape * 8;
  const failed = 63 + Math.floor(scrape / 3);
  const lines = [
    "# HELP gateway_observability_events_total Gateway lifecycle events processed by the background event consumer.",
    "# TYPE gateway_observability_events_total counter",
    `gateway_observability_events_total{kind="request"} ${requests}`,
    `gateway_observability_events_total{kind="attempt"} ${attempts}`,
    `gateway_observability_events_total{kind="usage"} ${requests - 12}`,
    `gateway_observability_events_total{kind="health"} ${18 + scrape}`,
    `gateway_observability_events_total{kind="diagnostic"} ${406 + scrape * 3}`,
    "# HELP gateway_observability_attempts_total Terminal upstream Attempts observed by outcome.",
    "# TYPE gateway_observability_attempts_total counter",
    `gateway_observability_attempts_total{outcome="succeeded"} ${attempts - failed}`,
    `gateway_observability_attempts_total{outcome="failed"} ${failed}`,
    "# TYPE gateway_observability_usage_tokens_total counter",
    `gateway_observability_usage_tokens_total{kind="input"} ${4_182_000 + scrape * 9100}`,
    `gateway_observability_usage_tokens_total{kind="output"} ${716_400 + scrape * 1400}`,
    `gateway_observability_usage_tokens_total{kind="reasoning"} ${233_900 + scrape * 620}`,
    `gateway_observability_usage_tokens_total{kind="cache_read"} ${1_905_200 + scrape * 4300}`,
    `gateway_observability_usage_tokens_total{kind="cache_creation"} ${88_600 + scrape * 210}`,
    `gateway_observability_usage_tokens_total{kind="cached"} ${1_993_800 + scrape * 4510}`,
    "# TYPE gateway_observability_queue_admission_total counter",
    'gateway_observability_queue_admission_total{outcome="required_queue_full"} 0',
    `gateway_observability_queue_admission_total{outcome="diagnostic_dropped"} ${142 + scrape * 2}`,
    'gateway_observability_queue_admission_total{outcome="sink_closed"} 0',
    "# TYPE gateway_observability_durable_events_total counter",
    'gateway_observability_durable_events_total{outcome="required_quarantined"} 0',
    'gateway_observability_durable_events_total{outcome="write_failed"} 0',
    "# TYPE gateway_observability_durable_pending_required gauge",
    `gateway_observability_durable_pending_required ${scrape % 5}`,
    "# TYPE gateway_observability_exports_total counter",
    `gateway_observability_exports_total{sink="json",outcome="emitted"} ${requests + attempts}`,
    'gateway_observability_exports_total{sink="opentelemetry",outcome="disabled"} 1',
  ];
  return `${lines.join("\n")}\n`;
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

    if (route === "POST /admin/access-groups") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const mismatch = requireDraftAndMatch(version, headers);
      if (mismatch !== undefined) return mismatch;
      const body = JSON.parse(bodyText ?? "{}") as GroupRow;
      const rows = state.groups.get(version.id) ?? [];
      if (rows.some((row) => row.id === body.id)) {
        return errorResponse(409, "management_lifecycle_conflict", "access group id already exists");
      }
      rows.push(body);
      state.groups.set(version.id, rows);
      version.revision += 1;
      return json(201, body, revisionToken(version));
    }

    const groupOne = /^(PATCH|DELETE) \/admin\/access-groups\/([^/]+)$/u.exec(route);
    if (groupOne !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const mismatch = requireDraftAndMatch(version, headers);
      if (mismatch !== undefined) return mismatch;
      const id = decodeURIComponent(groupOne[2] ?? "");
      const rows = state.groups.get(version.id) ?? [];
      const index = rows.findIndex((row) => row.id === id);
      if (index < 0) {
        return errorResponse(409, "management_lifecycle_conflict", "unknown access group");
      }
      if (groupOne[1] === "DELETE") {
        rows.splice(index, 1);
        state.groupRoutes.delete(`${version.id}:${id}`);
        version.revision += 1;
        return new Response(null, { status: 204 });
      }
      // PATCH takes the whole AccessGroupInput — replacement, not merge.
      const body = JSON.parse(bodyText ?? "{}") as GroupRow;
      rows[index] = { ...body, id };
      version.revision += 1;
      return json(200, rows[index], revisionToken(version));
    }

    const groupRoutes = /^(GET|POST) \/admin\/access-groups\/([^/]+)\/routes$/u.exec(route);
    if (groupRoutes !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const id = decodeURIComponent(groupRoutes[2] ?? "");
      const key = `${version.id}:${id}`;
      const rows = state.groupRoutes.get(key) ?? [];
      if (groupRoutes[1] === "GET") {
        return json(200, rows, revisionToken(version));
      }
      const mismatch = requireDraftAndMatch(version, headers);
      if (mismatch !== undefined) return mismatch;
      const body = JSON.parse(bodyText ?? "{}") as { route_id: string; enabled: boolean };
      const grant = { access_group_id: id, route_id: body.route_id, enabled: body.enabled };
      const existing = rows.findIndex((row) => row.route_id === body.route_id);
      if (existing >= 0) {
        rows[existing] = grant;
      } else {
        rows.push(grant);
      }
      state.groupRoutes.set(key, rows);
      version.revision += 1;
      return json(201, grant, revisionToken(version));
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
      const body = JSON.parse(bodyText ?? "{}") as Omit<RouteRow, "public_model_id">;
      const created: RouteRow = {
        id: body.id,
        public_model_id: modelId,
        policy: "smooth_weighted_round_robin",
        max_attempts: body.max_attempts,
        bootstrap_timeout_ms: body.bootstrap_timeout_ms,
      };
      routes.push(created);
      version.revision += 1;
      return json(201, created, revisionToken(version));
    }

    // ---- route workbench: get / patch / delete / candidates / validate ----
    //
    // validate must FAIL for a candidate-less route. The gateway does exactly
    // that (management_mutation_service.rs:2074), and a fixture that answered
    // `valid: true` here would make the dead end this workbench exists to fix
    // look like it never existed — the same way the OAuth fixture once
    // auto-completed and hid a wizard with no completion call.
    const routeCandidate = /^POST \/admin\/routes\/([^/]+)\/candidates$/u.exec(route);
    if (routeCandidate !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const mismatch = requireDraftAndMatch(version, headers);
      if (mismatch !== undefined) return mismatch;
      const routeId = decodeURIComponent(routeCandidate[1] ?? "");
      const routes = state.routes.get(version.id) ?? [];
      if (!routes.some((row) => row.id === routeId)) {
        return errorResponse(404, "management_resource_not_found", "no such route");
      }
      const rows =
        state.routeCandidates.get(version.id) ??
        state.routeCandidates.set(version.id, []).get(version.id) ??
        [];
      const body = JSON.parse(bodyText ?? "{}") as Omit<CandidateRow, "route_id">;
      if (rows.some((row) => row.id === body.id)) {
        return errorResponse(409, "management_lifecycle_conflict", "candidate id already exists");
      }
      const created: CandidateRow = { ...body, route_id: routeId };
      rows.push(created);
      version.revision += 1;
      return json(201, created, revisionToken(version));
    }

    const routeValidate = /^POST \/admin\/routes\/([^/]+)\/validate$/u.exec(route);
    if (routeValidate !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const routeId = decodeURIComponent(routeValidate[1] ?? "");
      const routes = state.routes.get(version.id) ?? [];
      if (!routes.some((row) => row.id === routeId)) {
        return errorResponse(404, "management_resource_not_found", "no such route");
      }
      const active = (state.routeCandidates.get(version.id) ?? []).filter(
        (row) => row.route_id === routeId && row.enabled,
      );
      const codes: string[] = [];
      if (active.length === 0) {
        codes.push("route_missing_active_candidate");
      }
      const endpoints = state.endpoints.get(version.id) ?? [];
      for (const candidate of active) {
        if (!endpoints.some((row) => row.id === candidate.endpoint_id)) {
          codes.push("route_candidate_endpoint_missing");
        }
      }
      codes.sort();
      // validateRoute declares no If-Match and does not advance the revision.
      return json(200, { valid: codes.length === 0, error_codes: [...new Set(codes)] });
    }

    const routeById = /^(GET|PATCH|DELETE) \/admin\/routes\/([^/]+)$/u.exec(route);
    if (routeById !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const routeId = decodeURIComponent(routeById[2] ?? "");
      const routes = state.routes.get(version.id) ?? [];
      const index = routes.findIndex((row) => row.id === routeId);
      if (index < 0) {
        return errorResponse(404, "management_resource_not_found", "no such route");
      }
      const current = routes[index] as RouteRow;
      if (routeById[1] === "GET") {
        return json(200, current, revisionToken(version));
      }
      const mismatch = requireDraftAndMatch(version, headers);
      if (mismatch !== undefined) return mismatch;
      if (routeById[1] === "DELETE") {
        routes.splice(index, 1);
        const candidates = state.routeCandidates.get(version.id) ?? [];
        state.routeCandidates.set(
          version.id,
          candidates.filter((row) => row.route_id !== routeId),
        );
        version.revision += 1;
        return new Response(null, {
          status: 204,
          headers: new Headers({ ETag: `"${revisionToken(version)}"` }),
        });
      }
      const body = JSON.parse(bodyText ?? "{}") as Omit<RouteRow, "public_model_id">;
      const updated: RouteRow = { ...body, public_model_id: current.public_model_id };
      routes[index] = updated;
      version.revision += 1;
      return json(200, updated, revisionToken(version));
    }

    // ---- subresource CRUD (real contract ops; PATCH replaces whole objects) ----
    const epCreate = /^POST \/admin\/upstreams\/([^/]+)\/endpoints$/u.exec(route);
    if (epCreate !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const mismatch = requireDraftAndMatch(version, headers);
      if (mismatch !== undefined) return mismatch;
      const body = JSON.parse(bodyText ?? "{}") as Omit<EndpointRow, "upstream_id">;
      const rows = state.endpoints.get(version.id) ?? [];
      if (rows.some((row) => row.id === body.id)) {
        return errorResponse(409, "management_lifecycle_conflict", "endpoint id already exists");
      }
      const row = { ...body, upstream_id: decodeURIComponent(epCreate[1] ?? "") } as EndpointRow;
      rows.push(row);
      state.endpoints.set(version.id, rows);
      version.revision += 1;
      return json(201, row, revisionToken(version));
    }

    const epOne = /^(GET|PATCH|DELETE) \/admin\/endpoints\/([^/]+)$/u.exec(route);
    if (epOne !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const id = decodeURIComponent(epOne[2] ?? "");
      const rows = state.endpoints.get(version.id) ?? [];
      const index = rows.findIndex((row) => row.id === id);
      if (index < 0) {
        return errorResponse(409, "management_lifecycle_conflict", "unknown endpoint");
      }
      if (epOne[1] === "GET") {
        return json(200, rows[index], revisionToken(version));
      }
      const mismatch = requireDraftAndMatch(version, headers);
      if (mismatch !== undefined) return mismatch;
      if (epOne[1] === "DELETE") {
        rows.splice(index, 1);
        state.bindings.set(
          version.id,
          (state.bindings.get(version.id) ?? []).filter((b) => b.endpoint_id !== id),
        );
        version.revision += 1;
        return new Response(null, { status: 204 });
      }
      const body = JSON.parse(bodyText ?? "{}") as EndpointRow;
      rows[index] = { ...body, id, upstream_id: rows[index]?.upstream_id ?? "" };
      version.revision += 1;
      return json(200, rows[index], revisionToken(version));
    }

    const credCreate = /^POST \/admin\/upstreams\/([^/]+)\/credentials$/u.exec(route);
    if (credCreate !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const mismatch = requireDraftAndMatch(version, headers);
      if (mismatch !== undefined) return mismatch;
      const body = JSON.parse(bodyText ?? "{}") as { id: string; kind: string; secret?: string; status: CredentialRow["status"] };
      if (body.secret === undefined || body.secret.length === 0) {
        return errorResponse(400, "invalid_management_request", "secret is required");
      }
      const rows = state.credentials.get(version.id) ?? [];
      if (rows.some((row) => row.id === body.id)) {
        return errorResponse(409, "management_lifecycle_conflict", "credential id already exists");
      }
      // The response is the redacted view: the secret never comes back out.
      const row: CredentialRow = {
        id: body.id,
        upstream_id: decodeURIComponent(credCreate[1] ?? ""),
        kind: body.kind,
        status: body.status,
        revision: 0,
        secret_present: true,
      };
      rows.push(row);
      state.credentials.set(version.id, rows);
      version.revision += 1;
      return json(201, row, revisionToken(version));
    }

    const credPatchDelete = /^(PATCH|DELETE) \/admin\/credentials\/([^/]+)$/u.exec(route);
    if (credPatchDelete !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const mismatch = requireDraftAndMatch(version, headers);
      if (mismatch !== undefined) return mismatch;
      const id = decodeURIComponent(credPatchDelete[2] ?? "");
      const rows = state.credentials.get(version.id) ?? [];
      const index = rows.findIndex((row) => row.id === id);
      const existing = rows[index];
      if (index < 0 || existing === undefined) {
        return errorResponse(409, "management_lifecycle_conflict", "unknown credential");
      }
      if (credPatchDelete[1] === "DELETE") {
        rows.splice(index, 1);
        state.bindings.set(
          version.id,
          (state.bindings.get(version.id) ?? []).filter((b) => b.credential_id !== id),
        );
        version.revision += 1;
        return new Response(null, { status: 204 });
      }
      const body = JSON.parse(bodyText ?? "{}") as { kind: string; secret?: string; status: CredentialRow["status"] };
      if (body.secret === undefined || body.secret.length === 0) {
        return errorResponse(400, "invalid_management_request", "secret is required on replace");
      }
      rows[index] = { ...existing, kind: body.kind, status: body.status, revision: existing.revision + 1 };
      version.revision += 1;
      return json(200, rows[index], revisionToken(version));
    }

    const bindCreate = /^POST \/admin\/endpoints\/([^/]+)\/credential-bindings$/u.exec(route);
    if (bindCreate !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const mismatch = requireDraftAndMatch(version, headers);
      if (mismatch !== undefined) return mismatch;
      const endpointId = decodeURIComponent(bindCreate[1] ?? "");
      const endpoint = (state.endpoints.get(version.id) ?? []).find((row) => row.id === endpointId);
      if (endpoint === undefined) {
        return errorResponse(409, "management_lifecycle_conflict", "unknown endpoint");
      }
      const body = JSON.parse(bodyText ?? "{}") as Omit<BindingRow, "endpoint_id" | "upstream_id">;
      const rows = state.bindings.get(version.id) ?? [];
      if (rows.some((b) => b.endpoint_id === endpointId && b.credential_id === body.credential_id)) {
        return errorResponse(409, "management_lifecycle_conflict", "binding already exists");
      }
      const row: BindingRow = { ...body, endpoint_id: endpointId, upstream_id: endpoint.upstream_id };
      rows.push(row);
      state.bindings.set(version.id, rows);
      version.revision += 1;
      return json(201, row, revisionToken(version));
    }

    // ---- P13-04A: operational account-pool inventory (real contract op) ----
    // One row per endpoint-credential binding, joined up through channel and
    // provider. URL-free by contract: no base_url, no inference_path.
    if (route === "GET /admin/operations/account-pools") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const providerFilter = url.searchParams.get("provider_id");
      const channelFilter = url.searchParams.get("channel_id");
      const statusFilter = url.searchParams.get("account_status");
      const enabledFilter = url.searchParams.get("enabled");

      const upstreams = state.upstreams.get(version.id) ?? [];
      const endpoints = state.endpoints.get(version.id) ?? [];
      const credentials = state.credentials.get(version.id) ?? [];
      const routes = state.routes.get(version.id) ?? [];

      const items = (state.bindings.get(version.id) ?? [])
        .map((binding) => {
          const channel = endpoints.find((row) => row.id === binding.endpoint_id);
          const account = credentials.find((row) => row.id === binding.credential_id);
          const provider = upstreams.find((row) => row.id === binding.upstream_id);
          if (channel === undefined || account === undefined || provider === undefined) {
            return undefined;
          }
          // The operations plane carries its own status vocabulary
          // (active|cooling|unauthorized|disabled) — the stored config value
          // is mapped, not reused, so the fixture exercises the real set.
          const accountStatus =
            account.status === "active"
              ? account.kind === "oauth"
                ? "cooling"
                : "active"
              : account.status === "revoked"
                ? "unauthorized"
                : "disabled";
          return {
            provider_id: provider.id,
            provider_name: provider.name ?? provider.id,
            provider_kind: provider.kind,
            provider_enabled: provider.enabled,
            egress_policy_id: provider.egress_policy_id ?? null,
            channel_id: channel.id,
            adapter_id: channel.adapter_id,
            api_format: channel.api_format,
            transport: "http",
            channel_enabled: channel.enabled,
            account_id: account.id,
            account_kind: account.kind,
            account_status: accountStatus,
            account_revision: account.revision,
            binding_enabled: binding.enabled,
            configured_enabled: provider.enabled && channel.enabled && binding.enabled,
            priority: binding.priority,
            weight: binding.weight,
            concurrency: binding.concurrency,
            route_ids: routes.map((row) => row.id),
          };
        })
        .filter((row) => row !== undefined)
        .filter((row) => providerFilter === null || row.provider_id === providerFilter)
        .filter((row) => channelFilter === null || row.channel_id === channelFilter)
        .filter((row) => statusFilter === null || row.account_status === statusFilter)
        .filter(
          (row) => enabledFilter === null || row.configured_enabled === (enabledFilter === "true"),
        );

      return json(200, {
        config_version_id: version.id,
        revision: version.revision,
        items,
        next_cursor: null,
      });
    }

    // ---- operations/usage (P13-04B) ----
    //
    // Deterministic generated rows, because the shape's hard parts only show up
    // at scale: `limit` caps at 100, so the page must follow the cursor before
    // it can add anything up. A fixture returning one tidy page would let a
    // first-page-only sum look correct.
    //
    // The row set deliberately contains unobserved totals (`total: null`) and
    // mixed confidences — those are the cases the page must not smooth over.
    if (route === "GET /admin/operations/usage") {
      // NOT version-scoped: the contract declares no X-Config-Version for this
      // operation, so requiring one here would have the fixture reject requests
      // the real gateway accepts.
      const providers = ["prov-relay-a", "prov-grok"];
      const channels = ["ch-relay-responses", "ch-grok-build"];
      const models = ["minimax-m3", "glm-5-air", "grok-4"];
      const protocols = [
        "openai_chat_completions",
        "openai_responses",
        "anthropic_messages",
      ] as const;
      // `prov-flood` is a deliberate over-cap case so the truncation path is
      // reachable in dev and in E2E; it exists only when asked for by name.
      const flood = url.searchParams.get("provider_id") === "prov-flood";
      const total = flood ? 2400 : 137;

      const all = Array.from({ length: total }, (_, index) => {
        const provider = flood ? "prov-flood" : (providers[index % providers.length] as string);
        // every 11th row reports no input observation at all
        const unobserved = index % 11 === 0;
        // every 5th row is a partial observation
        const partial = index % 5 === 0;
        const confidence = unobserved ? "unknown" : partial ? "partial" : "exact";
        const family = (base: number) => ({
          total: unobserved ? null : base * (index + 1),
          confidence,
        });
        return {
          provider_id: provider,
          channel_id: flood ? "ch-flood" : (channels[index % channels.length] as string),
          account_id: `acct-${index % 7}`,
          public_model: models[index % models.length] as string,
          protocol: protocols[index % protocols.length],
          client_key_id: `ck-${index % 4}`,
          // A Client Key need not belong to a group; null is a real value the
          // page must bucket rather than drop.
          access_group_id: index % 6 === 0 ? null : `ag-${index % 3}`,
          request_count: (index % 17) + 1,
          usage_observations: (index % 17) + 1,
          input_tokens: family(120),
          output_tokens: family(40),
          reasoning_tokens: family(index % 3 === 0 ? 0 : 12),
          cache_read_tokens: family(0),
          cache_creation_tokens: family(0),
          cached_tokens: family(0),
          observed_at_ms: 1787000000000 + index * 1000,
          cost_microunits: null,
          cost_confidence: "unpriced" as const,
        };
      });

      const filtered = all.filter((row) => {
        for (const [param, field] of [
          ["provider_id", "provider_id"],
          ["channel_id", "channel_id"],
          ["account_id", "account_id"],
          ["model", "public_model"],
          ["client_key_id", "client_key_id"],
          ["access_group_id", "access_group_id"],
          ["protocol", "protocol"],
        ] as const) {
          const want = url.searchParams.get(param);
          if (want !== null && String(row[field] ?? "") !== want) {
            return false;
          }
        }
        return true;
      });

      const limit = Math.min(Number(url.searchParams.get("limit") ?? 50) || 50, 100);
      const offset = Number(url.searchParams.get("cursor") ?? 0) || 0;
      const slice = filtered.slice(offset, offset + limit);
      const nextOffset = offset + slice.length;
      return json(200, {
        observed_through_ms: filtered.length === 0 ? null : 1787000600000,
        items: slice,
        next_cursor: nextOffset < filtered.length ? String(nextOffset) : null,
      });
    }

    // ---- credential detail + metadata (real contract ops) ----
    const credGet = /^GET \/admin\/credentials\/([^/]+)$/u.exec(route);
    if (credGet !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const id = decodeURIComponent(credGet[1] ?? "");
      const row = (state.credentials.get(version.id) ?? []).find((entry) => entry.id === id);
      if (row === undefined) {
        return errorResponse(409, "management_lifecycle_conflict", "unknown credential");
      }
      return json(200, row);
    }
    const credMeta = /^GET \/admin\/credentials\/([^/]+)\/metadata$/u.exec(route);
    if (credMeta !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const id = decodeURIComponent(credMeta[1] ?? "");
      const row = (state.credentials.get(version.id) ?? []).find((entry) => entry.id === id);
      if (row === undefined) {
        return errorResponse(409, "management_lifecycle_conflict", "unknown credential");
      }
      // Every metadata field is nullable in the contract. The oauth credential
      // carries a full account identity; the api_key one carries almost
      // nothing — both are real shapes and the UI has to read honestly.
      const rich = row.kind === "oauth";
      return json(200, {
        credential_id: row.id,
        kind: row.kind,
        revision: row.revision,
        plan: rich ? "SuperGrok Heavy" : null,
        quota: rich ? "1000 req/day" : null,
        platform: rich ? "grok" : null,
        email: rich ? "ops@fixture.example" : null,
        source_format: rich ? "direct_oauth" : null,
      });
    }
    const oauthRefresh = /^POST \/admin\/credentials\/([^/]+)\/oauth\/refresh$/u.exec(route);
    if (oauthRefresh !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const id = decodeURIComponent(oauthRefresh[1] ?? "");
      const row = (state.credentials.get(version.id) ?? []).find((entry) => entry.id === id);
      if (row === undefined || row.kind !== "oauth") {
        return errorResponse(409, "management_lifecycle_conflict", "credential does not hold an oauth token");
      }
      row.revision += 1;
      return json(202, { credential_id: id, state: "complete", revision: row.revision });
    }

    // ---- credential OAuth (real contract ops; authorization-code flow) ----
    const oauthStart = /^POST \/admin\/credentials\/([^/]+)\/oauth\/start$/u.exec(route);
    if (oauthStart !== null) {
      const id = decodeURIComponent(oauthStart[1] ?? "");
      const authState = `st-${hashString(id).toString(16)}`;
      const op: OAuthOp = {
        state: "pending",
        polls: 0,
        expires_at_ms: Date.now() + 300_000,
        authState,
        authorization_url:
          `https://auth.fixture.example/authorize?client_id=prism-fixture` +
          `&redirect_uri=${encodeURIComponent("http://127.0.0.1:8085/callback")}` +
          `&response_type=code&scope=openid+offline&state=${authState}`,
      };
      state.oauthOps.set(id, op);
      return json(202, {
        credential_id: id,
        state: op.state,
        expires_at_ms: op.expires_at_ms,
        authorization_url: op.authorization_url,
      });
    }
    const oauthStatus = /^GET \/admin\/credentials\/([^/]+)\/oauth\/status$/u.exec(route);
    if (oauthStatus !== null) {
      const id = decodeURIComponent(oauthStatus[1] ?? "");
      const op = state.oauthOps.get(id);
      if (op === undefined) {
        return errorResponse(409, "management_lifecycle_conflict", "no oauth operation for credential");
      }
      // Deliberately does NOT auto-complete. The old fixture flipped to
      // complete on the third poll, which made a wizard with no completion
      // call look like it worked — the real flow cannot finish without the
      // callback, and the fixture must not be kinder than the gateway.
      op.polls += 1;
      return json(200, {
        credential_id: id,
        state: op.state,
        expires_at_ms: op.expires_at_ms,
        authorization_url: op.authorization_url,
        ...(op.failure_class === undefined ? {} : { failure_class: op.failure_class }),
      });
    }
    const oauthCallback = /^POST \/admin\/credentials\/([^/]+)\/oauth\/callback$/u.exec(route);
    if (oauthCallback !== null) {
      const id = decodeURIComponent(oauthCallback[1] ?? "");
      const op = state.oauthOps.get(id);
      if (op === undefined || op.state !== "pending") {
        return errorResponse(409, "management_lifecycle_conflict", "no pending oauth operation");
      }
      const body = JSON.parse(bodyText ?? "{}") as { state?: string; code?: string; error?: string };
      if (body.state !== op.authState) {
        op.state = "failed";
        op.failure_class = "state_mismatch";
      } else if (body.error !== undefined) {
        op.state = "failed";
        op.failure_class = "provider_rejected";
      } else {
        op.state = "complete";
        const row = (state.credentials.get("draft-2026-08") ?? []).find((entry) => entry.id === id);
        if (row !== undefined) {
          row.status = "active";
          row.revision += 1;
        }
      }
      return json(202, {
        credential_id: id,
        state: op.state,
        expires_at_ms: op.expires_at_ms,
        ...(op.failure_class === undefined ? {} : { failure_class: op.failure_class }),
      });
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

    // ---- PROPOSED G3: analytics + dashboard summary (deterministic demo data) ----
    if (route === "POST /admin/analytics") {
      const query = JSON.parse(bodyText ?? "{}") as {
        from_ms: number;
        to_ms: number;
        bucket?: string;
        filters?: {
          status?: string;
          public_model?: string[];
          client_key_id?: string[];
          credential_id?: string[];
          endpoint_id?: string[];
        };
        include?: {
          summary?: boolean;
          timeline?: boolean;
          ranks?: { by: string; limit: number };
          heatmap?: { metric: string };
          options?: boolean;
          events?: { cursor: string | null; limit: number };
        };
      };
      const span = query.to_ms - query.from_ms;
      const bucketMs = (query.bucket === "day" || (query.bucket !== "hour" && span > 48 * 3_600_000))
        ? 86_400_000
        : 3_600_000;
      const bucketCount = Math.max(1, Math.min(400, Math.ceil(span / bucketMs)));

      // A single-value filter on any entity dimension is what the comparison
      // chart sends, one query per series. Without segmenting here every series
      // would come back identical and the chart would look like a bug in the UI
      // rather than a gap in the fixture. `share` is derived from the pinned
      // identifier so it is stable across refreshes, and the shares of a
      // dimension's known values sum to roughly 1.
      const pinned =
        query.filters?.public_model?.[0] ??
        query.filters?.client_key_id?.[0] ??
        query.filters?.credential_id?.[0] ??
        query.filters?.endpoint_id?.[0];
      const share = pinned === undefined ? 1 : 0.22 + noise(hashString(pinned)) * 0.5;
      const phase = pinned === undefined ? 0 : Math.floor(noise(hashString(pinned) + 5) * 11);

      const buckets = Array.from({ length: bucketCount }, (_, index) => {
        const requests = Math.max(
          0,
          Math.round((40 + noise(index + phase) * 180) * share),
        );
        const failures = Math.round(noise(index * 7 + 3 + phase) * 6 * share);
        return {
          bucket_start_ms: query.from_ms + index * bucketMs,
          requests,
          failures,
          tokens_total: requests * Math.round(9000 + noise(index + 99) * 6000),
          latency_p95_ms: Math.round(8000 + noise(index + 7) * 26000),
        };
      });
      const requests = buckets.reduce((sum, bucket) => sum + bucket.requests, 0);
      const failures = buckets.reduce((sum, bucket) => sum + bucket.failures, 0);
      const tokensTotal = buckets.reduce((sum, bucket) => sum + bucket.tokens_total, 0);
      const models = ["minimax-m3", "glm-5-air"];
      // More than COMPARE_LIMIT entries in the two new dimensions, so the top-N
      // slice is a real slice rather than "everything there is".
      const clientKeyIds = ["key-cli", "key-web", "key-ci", "key-mobile", "key-batch"];
      const credentialIds = ["cred-relay-key", "cred-grok-oauth", "cred-kiro-sub", "cred-grok-build"];
      const endpointIds = ["ep-relay-a-responses", "ep-grok-build"];
      const body: Record<string, unknown> = {
        range: { from_ms: query.from_ms, to_ms: query.to_ms, bucket: bucketMs === 3_600_000 ? "hour" : "day", bucket_count: bucketCount },
      };
      if (query.include?.summary === true) {
        body["summary"] = {
          requests,
          failures,
          attempts: requests + failures,
          tokens: {
            input: Math.round(tokensTotal * 0.12),
            output: Math.round(tokensTotal * 0.006),
            reasoning: Math.round(tokensTotal * 0.002),
            cache_read: Math.round(tokensTotal * 0.87),
            cache_creation: Math.round(tokensTotal * 0.002),
          },
          latency_ms: { avg: 12400, p50: 9800, p95: 21400, p99: 39000 },
        };
      }
      if (query.include?.timeline === true) {
        body["timeline"] = buckets;
      }
      if (query.include?.ranks !== undefined) {
        // Honour `by`: the Client Key and credential tabs ask for their own
        // dimension, and returning models for all three would make the new tabs
        // look wired when they are not.
        const dimension = query.include.ranks.by;
        const pool =
          dimension === "client_key"
            ? clientKeyIds
            : dimension === "credential"
              ? credentialIds
              : dimension === "endpoint"
                ? endpointIds
                : models;
        const weights = pool.map((key) => 0.2 + noise(hashString(key)) * 0.8);
        const weightTotal = weights.reduce((sum, weight) => sum + weight, 0);
        body["ranks"] = pool
          .map((key, index) => {
            const fraction = (weights[index] ?? 0) / weightTotal;
            return {
              key,
              requests: Math.round(requests * fraction),
              failures: Math.round(failures * fraction),
              tokens_total: Math.round(tokensTotal * fraction),
              last_seen_ms: query.to_ms - 60_000 - index * 30_000,
            };
          })
          .sort((a, b) => b.requests - a.requests)
          .slice(0, query.include.ranks.limit);
      }
      if (query.include?.heatmap !== undefined) {
        const cells = [];
        for (let weekday = 0; weekday < 7; weekday += 1) {
          for (let hour = 0; hour < 24; hour += 1) {
            cells.push({ weekday, hour, value: Math.round(noise(weekday * 24 + hour) * 200) });
          }
        }
        body["heatmap"] = cells;
      }
      if (query.include?.options === true) {
        body["options"] = {
          public_model: models,
          client_key_id: clientKeyIds,
          credential_id: credentialIds,
          endpoint_id: endpointIds,
        };
      }
      if (query.include?.events !== undefined) {
        const TOTAL = 57;
        const offset = query.include.events.cursor === null ? 0 : Number(query.include.events.cursor);
        const limit = Math.min(query.include.events.limit, 1000);
        const statusFilter = query.filters?.status ?? "all";
        const all = Array.from({ length: TOTAL }, (_, index) => {
          const failed = index % 7 === 3;
          return {
            request_id: `req-${String(1000 + TOTAL - index)}`,
            occurred_at_ms: query.to_ms - index * 97_000,
            protocol: index % 3 === 0 ? "anthropic_messages" : "openai_responses",
            public_model: models[index % 2 === 0 ? 0 : 1],
            streaming: index % 4 !== 1,
            outcome: failed ? "failed" : "success",
            error_code: failed ? "provider_rate_limited" : null,
            error_scope: failed ? "quota_window" : null,
            stage: failed ? "http_status" : null,
            retry_decision: failed ? "RetryClosed" : "Completed",
            attempt_count: failed ? 2 : 1,
            latency_ms: Math.round(4000 + noise(index) * 30000),
            tokens: failed ? null : { input: 8000 + index * 13, output: 450 + index, cache_read: 60000 + index * 31 },
            client_key_id: "key-cli",
            credential_id: "cred-relay-key",
            endpoint_id: "ep-relay-a-responses",
          };
        }).filter((row) => statusFilter === "all" || row.outcome === statusFilter);
        const page = all.slice(offset, offset + limit);
        body["events"] = {
          items: page,
          next_cursor: offset + limit < all.length ? String(offset + limit) : null,
        };
      }
      return json(200, body);
    }

    if (route === "GET /admin/dashboard/summary") {
      const todayStart = Number(url.searchParams.get("today_start_ms") ?? Date.now() - 8 * 3_600_000);
      const now = Number(url.searchParams.get("now_ms") ?? Date.now());
      const strip = [];
      for (let start = todayStart; start < now; start += 600_000) {
        const index = Math.floor((start - todayStart) / 600_000);
        const roll = noise(index + 42);
        strip.push({
          bucket_start_ms: start,
          state: roll < 0.08 ? "empty" : roll > 0.97 ? "bad" : roll > 0.9 ? "warn" : "ok",
        });
      }
      return json(200, {
        kpi: { requests: 1877, failures: 24, success_rate: 0.9872, tokens_total: 217_600_000, latency_p95_ms: 21_400 },
        health_strip: strip,
        token_mix: { input: 27_100_000, output: 1_000_000, reasoning: 336_100, cache_read: 189_200_000, cache_creation: 400_000 },
        top_models: [
          { public_model: "minimax-m3", requests: 1351, tokens_total: 174_000_000 },
          { public_model: "glm-5-air", requests: 526, tokens_total: 43_600_000 },
        ],
        recent_failures: [
          { request_id: "req-1043", occurred_at_ms: now - 340_000, error_code: "provider_rate_limited", error_scope: "quota_window", stage: "http_status" },
          { request_id: "req-1029", occurred_at_ms: now - 1_960_000, error_code: "stream_truncated", error_scope: "stream", stage: "sse_bootstrap" },
        ],
      });
    }

    // ---- observability exposition (real contract op, text/plain) ----
    if (route === "GET /admin/observability/metrics") {
      scrapes += 1;
      return new Response(renderMetrics(scrapes), {
        status: 200,
        headers: new Headers({ "Content-Type": "text/plain; version=0.0.4" }),
      });
    }

    // ---- runtime projections (real contract ops) ----
    if (route === "GET /admin/runtime/availability") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const rows = [
        { endpoint_id: "ep-relay-a-responses", credential_id: "cred-relay-key", availability: "available" },
        { endpoint_id: "ep-relay-a-responses", credential_id: "cred-grok-oauth", availability: "cooldown" },
        { endpoint_id: "ep-grok-build", credential_id: "cred-grok-oauth", availability: "credential_forbidden" },
        { endpoint_id: "ep-grok-build", credential_id: "cred-relay-key", availability: "quota_blocked" },
      ];
      return json(200, rows, revisionToken(version));
    }
    if (route === "GET /admin/catalog/status") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const now = 1785100000000;
      return json(
        200,
        [
          { endpoint_id: "ep-relay-a-responses", credential_id: "cred-relay-key", freshness: "fresh", observed_at_ms: now - 900_000 },
          { endpoint_id: "ep-relay-a-responses", credential_id: "cred-grok-oauth", freshness: "stale", observed_at_ms: now - 30 * 3_600_000 },
          { endpoint_id: "ep-grok-build", credential_id: "cred-grok-oauth", freshness: "missing", observed_at_ms: 0 },
        ],
        revisionToken(version),
      );
    }
    if (route === "POST /admin/runtime/quota/reset") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const body = JSON.parse(bodyText ?? "{}") as { credential_id?: string };
      return json(202, {
        state: body.credential_id === "cred-grok-oauth" ? "probe_scheduled" : "rejected",
      });
    }
    const explain = /^GET \/admin\/routes\/([^/]+)\/explain$/u.exec(route);
    if (explain !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      // Explain resolves against a COMPILED SNAPSHOT, and only a published
      // version has one (apps/gateway/src/runtime.rs::explain_route →
      // snapshot_for). A draft 503s on the real gateway even when its route is
      // perfectly valid — measured 2026-08-18. Answering 200 here would make a
      // draft-only deployment look like Explain works, the same way the OAuth
      // fixture once auto-completed and hid a wizard with no completion call.
      if (version.status !== "active") {
        return errorResponse(
          503,
          "management_runtime_unavailable",
          "no compiled snapshot for a draft version",
        );
      }
      const routeId = decodeURIComponent(explain[1] ?? "");
      // A multi-Provider route fails closed without provider_id (P13-07B). The
      // fixture reproduces that rather than answering anyway, so the selector
      // is exercised instead of merely rendered.
      const providerId = url.searchParams.get("provider_id");
      if (routeId === "rt-multi-provider" && providerId === null) {
        return errorResponse(
          409,
          "provider_scope_required",
          "route spans multiple providers; provider_id is required",
        );
      }
      const stored = (state.routeCandidates.get(version.id) ?? []).filter(
        (row) => row.route_id === routeId,
      );
      const candidates =
        stored.length > 0
          ? stored.map((row, index) => ({
              candidate_id: row.id,
              decision: row.enabled && index === 0 ? "selected" : "excluded",
              ...(row.enabled ? {} : { reason: "CandidateDisabled" }),
              price_evidence: index === 0 ? "dominant" : "dominated",
            }))
          : [
              {
                candidate_id: "cand-relay-primary",
                decision: "selected",
                price_evidence: "dominant",
              },
              {
                candidate_id: "cand-grok-fallback",
                decision: "excluded",
                reason: "NoEligibleCredential",
                price_evidence: "unpriced",
              },
            ];
      return json(200, {
        route_id: routeId,
        // Required and nullable: null means the policy is disabled.
        price_policy:
          routeId === "rt-unpriced"
            ? null
            : { catalog_version_id: "cat-2026-08", comparison: "rate_dominance_v1" },
        candidates,
      });
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
