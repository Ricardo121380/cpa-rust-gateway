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

type CompatPoolRow = { id: string; upstream_id: string; name: string; enabled: boolean };

type CompatNodeRow = {
  id: string;
  upstream_id: string;
  pool_id: string | null;
  name: string;
  enabled: boolean;
  weight: number;
  maximum_concurrency: number;
  proxy_configured: boolean;
};

type CompatBindingRow = {
  endpoint_id: string;
  credential_id: string;
  target_kind: string;
  target_id: string | null;
  failure_scope: string;
  stickiness: string;
  pre_submit_max_attempts: number;
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

type CatalogEntryRow = {
  provider_id: string;
  channel_id: string;
  model: string;
  input_microunits_per_million: number;
  output_microunits_per_million: number;
  reasoning_microunits_per_million: number;
  cache_read_microunits_per_million: number;
  cache_creation_microunits_per_million: number;
  cached_microunits_per_million: number;
};

type CatalogRow = {
  catalog_version_id: string;
  effective_at_ms: number;
  created_at_ms: number;
  source: string;
  entries: CatalogEntryRow[];
};

/** Pinned so "is this catalog effective yet" is deterministic in tests. */
const FIXTURE_NOW_MS = 1787100000000;

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
  // GLOBAL, not keyed by config version — that is the contract's shape.
  catalogs: [
    {
      catalog_version_id: "cat-2026-07",
      effective_at_ms: 1784000000000,
      created_at_ms: 1783900000000,
      source: "imported",
      entries: [
        {
          provider_id: "relay-a",
          channel_id: "ep-relay-a-responses",
          model: "minimax-m3",
          input_microunits_per_million: 1400000,
        output_microunits_per_million: 5600000,
        reasoning_microunits_per_million: 0,
        cache_read_microunits_per_million: 0,
        cache_creation_microunits_per_million: 0,
        cached_microunits_per_million: 0,
        },
      ],
    },
    {
      catalog_version_id: "cat-2026-08",
      effective_at_ms: 1786000000000,
      created_at_ms: 1785900000000,
      source: "operator",
      entries: [
        {
          provider_id: "relay-a",
          channel_id: "ep-relay-a-responses",
          model: "minimax-m3",
          input_microunits_per_million: 1500000,
        output_microunits_per_million: 6000000,
        reasoning_microunits_per_million: 0,
        cache_read_microunits_per_million: 0,
        cache_creation_microunits_per_million: 0,
        cached_microunits_per_million: 0,
        },
        {
          provider_id: "grok-build-pool",
          channel_id: "ep-grok-build",
          model: "grok-4",
          input_microunits_per_million: 3000000,
        output_microunits_per_million: 15000000,
        reasoning_microunits_per_million: 0,
        cache_read_microunits_per_million: 0,
        cache_creation_microunits_per_million: 0,
        cached_microunits_per_million: 0,
        },
      ],
    },
    {
      // Dated ahead of FIXTURE_NOW_MS: listed, but binding it must fail.
      catalog_version_id: "cat-2026-09-preview",
      effective_at_ms: 1790000000000,
      created_at_ms: 1786500000000,
      source: "operator",
      entries: [
        {
          provider_id: "relay-a",
          channel_id: "ep-relay-a-responses",
          model: "minimax-m3",
          input_microunits_per_million: 1200000,
        output_microunits_per_million: 4800000,
        reasoning_microunits_per_million: 0,
        cache_read_microunits_per_million: 0,
        cache_creation_microunits_per_million: 0,
        cached_microunits_per_million: 0,
        },
      ],
    },
  ] as CatalogRow[],
  pricePolicy: new Map<string, { catalog_version_id: string; comparison: string }>(),
  poolSnapshot: 1,
  // Compatible proxy pools / nodes / bindings (P13-11 A–D). Seeded so the two
  // states that matter are both on screen from the start: a pool WITH nodes,
  // and a pool with NONE — the latter being exactly what a freshly created
  // pool looks like, and the reason the create button is section-level.
  compatPools: new Map<string, CompatPoolRow[]>([
    [
      "draft-2026-08",
      [
        { id: "pool-eu", upstream_id: "relay-a", name: "EU 出口池", enabled: true },
        { id: "pool-empty", upstream_id: "relay-a", name: "空池(刚建的样子)", enabled: true },
      ],
    ],
  ]),
  compatNodes: new Map<string, CompatNodeRow[]>([
    [
      "draft-2026-08",
      [
        // proxy_configured is hardcoded true by the backend: a stored node
        // always has a sealed endpoint. The fixture does not invent a false.
        { id: "node-eu-1", upstream_id: "relay-a", pool_id: "pool-eu", name: "法兰克福 1",
          enabled: true, weight: 1, maximum_concurrency: 8, proxy_configured: true },
        { id: "node-eu-2", upstream_id: "relay-a", pool_id: "pool-eu", name: "阿姆斯特丹 1",
          enabled: false, weight: 2, maximum_concurrency: 4, proxy_configured: true },
        { id: "node-loose", upstream_id: "grok-build-pool", pool_id: null, name: "独立节点",
          enabled: true, weight: 1, maximum_concurrency: 2, proxy_configured: true },
      ],
    ],
  ]),
  compatBindings: new Map<string, CompatBindingRow[]>([
    [
      "draft-2026-08",
      [
        { endpoint_id: "ep-relay-a-responses", credential_id: "cred-relay-key",
          target_kind: "proxy_pool", target_id: "pool-eu", failure_scope: "egress_node",
          stickiness: "credential", pre_submit_max_attempts: 2 },
        { endpoint_id: "ep-grok-build", credential_id: "cred-grok-oauth",
          target_kind: "fixed_proxy", target_id: "node-eu-1", failure_scope: "credential",
          stickiness: "credential_and_egress", pre_submit_max_attempts: 1 },
      ],
    ],
  ]),
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

/** The backend's admitted pairs, reproduced so the fixture rejects what the
 *  gateway rejects: ("direct", null) | ("fixed_proxy", id) | ("proxy_pool", id). */
function validCompatTarget(kind: string, targetId: string | null): boolean {
  if (kind === "direct") {
    return targetId === null;
  }
  return (kind === "fixed_proxy" || kind === "proxy_pool") && targetId !== null && targetId !== "";
}

/** Mirrors UpstreamProxy::try_socks5 — bare socks5://host:port, nothing else. */
function validSocks5(value: string | null | undefined): boolean {
  if (typeof value !== "string") {
    return false;
  }
  try {
    const url = new URL(value);
    return (
      url.protocol === "socks5:" &&
      url.username === "" &&
      url.password === "" &&
      url.hostname !== "" &&
      url.port !== "" &&
      (url.pathname === "" || url.pathname === "/") &&
      url.search === "" &&
      url.hash === ""
    );
  } catch {
    return false;
  }
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

    // ---- operations/billing + failure attribution + per-request attempts ----
    //
    // Three sources, three different scoping rules — reproduced exactly,
    // because getting them wrong is precisely how the page would mislead:
    //   billing   NOT version-scoped, carries a whole-window `summary`
    //   failures  IS version-scoped
    //   attempts  NOT version-scoped, bare array, no paging
    if (route === "GET /admin/operations/billing") {
      const models = ["minimax-m3", "glm-5-air", "grok-4"];
      const confidences = ["exact", "exact", "partial", "unknown", "unpriced"] as const;
      const all = Array.from({ length: 73 }, (_, index) => {
        const confidence = confidences[index % confidences.length] as string;
        const priced = confidence === "exact" || confidence === "partial";
        const observed = confidence !== "unknown";
        return {
          ledger_id: 1000 + index,
          request_id: `req-${1000 + index}`,
          response_id: `resp-${1000 + index}`,
          provider_id: index % 2 === 0 ? "prov-relay-a" : "prov-grok",
          channel_id: index % 2 === 0 ? "ch-relay-responses" : "ch-grok-build",
          account_id: `acct-${index % 3}`,
          model: models[index % models.length] as string,
          input_tokens: observed ? 100 + index : null,
          output_tokens: observed ? 20 + index : null,
          reasoning_tokens: null,
          cache_read_tokens: observed ? 0 : null,
          cache_creation_tokens: observed ? 0 : null,
          cached_tokens: observed ? 0 : null,
          occurred_at_ms: 1787000000000 + index * 60000,
          catalog_version_id: priced ? "cat-2026-08" : null,
          cost_microunits: priced ? 4200 + index * 7 : null,
          cost_confidence: confidence,
        };
      });

      const filtered = all.filter((row) => {
        for (const [param, field] of [
          ["provider_id", "provider_id"],
          ["channel_id", "channel_id"],
          ["account_id", "account_id"],
          ["model", "model"],
          ["status", "cost_confidence"],
        ] as const) {
          const want = url.searchParams.get(param);
          if (want !== null && String(row[field] ?? "") !== want) {
            return false;
          }
        }
        return true;
      });

      // The summary is computed over the WHOLE filtered set before the cursor
      // applies — that is what the backend does, and the page relies on it to
      // show correct figures from page one.
      const summary = {
        records: filtered.length,
        exact_records: filtered.filter((row) => row.cost_confidence === "exact").length,
        partial_records: filtered.filter((row) => row.cost_confidence === "partial").length,
        unknown_records: filtered.filter((row) => row.cost_confidence === "unknown").length,
        unpriced_records: filtered.filter((row) => row.cost_confidence === "unpriced").length,
        known_cost_microunits: filtered.reduce((sum, row) => sum + (row.cost_microunits ?? 0), 0),
      };

      const limit = Math.min(Number(url.searchParams.get("limit") ?? 50) || 50, 100);
      const offset = Number(url.searchParams.get("cursor") ?? 0) || 0;
      const slice = filtered.slice(offset, offset + limit);
      return json(200, {
        snapshot_ledger_id: all.at(-1)?.ledger_id ?? null,
        items: slice,
        summary,
        next_cursor: offset + slice.length < filtered.length ? String(offset + slice.length) : null,
      });
    }

    if (route === "GET /admin/operations/provider-account-pools/failures") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const codes = [
        "ProviderRateLimited",
        "CredentialQuotaExceeded",
        "EgressUnavailable",
        "ProviderTransient",
        "UpstreamProtocolError",
      ];
      const scopes = ["provider", "quota_window", "egress", "provider", "stream"];
      const decisions = [
        "retry_eligible",
        "non_retryable",
        "retry_closed",
        "retry_eligible",
        "completed",
      ];
      const all = Array.from({ length: 47 }, (_, index) => ({
        provider_id: index % 2 === 0 ? "prov-relay-a" : "prov-grok",
        channel_id: index % 2 === 0 ? "ch-relay-responses" : "ch-grok-build",
        account_id: `acct-${index % 3}`,
        // The same request appears more than once: a request can produce
        // several failed attempts, which is why row count != failed requests.
        request_id: `req-${1000 + Math.floor(index / 2)}`,
        attempt_id: `att-${index}`,
        ended_at_ms: 1787000000000 + index * 30000,
        error_code: codes[index % codes.length] as string,
        error_scope: scopes[index % scopes.length] as string,
        retry_decision: decisions[index % decisions.length] as string,
      }));
      const filtered = all.filter((row) => {
        for (const param of ["provider_id", "channel_id", "account_id"] as const) {
          const want = url.searchParams.get(param);
          if (want !== null && row[param] !== want) {
            return false;
          }
        }
        return true;
      });
      const limit = Math.min(Number(url.searchParams.get("limit") ?? 50) || 50, 100);
      const offset = Number(url.searchParams.get("cursor") ?? 0) || 0;
      const slice = filtered.slice(offset, offset + limit);
      return json(200, {
        observed_through_ordinal: filtered.length === 0 ? null : filtered.length,
        items: slice,
        next_cursor: offset + slice.length < filtered.length ? String(offset + slice.length) : null,
      });
    }

    const attempts = /^GET \/admin\/requests\/([^/]+)\/attempts$/u.exec(route);
    if (attempts !== null) {
      const requestId = decodeURIComponent(attempts[1] ?? "");
      // A bare array by contract: no cursor, no envelope. `outcome` is a free
      // string, not an enum, so the fixture returns values the UI must not try
      // to map to a closed vocabulary.
      return json(200, [
        {
          attempt_id: `${requestId}-a0`,
          outcome: "provider_rate_limited",
          stage: "http_status",
          endpoint_id: "ep-relay-a-responses",
          credential_id: "cred-relay-key",
        },
        {
          attempt_id: `${requestId}-a1`,
          outcome: "succeeded",
          stage: "sse_bootstrap",
          endpoint_id: "ep-relay-a-responses",
          credential_id: "cred-grok-oauth",
        },
      ]);
    }

    // ---- billing catalogs + routing price policy (P13-05C / P13-07D) ----
    //
    // Two different scopes on one page, reproduced faithfully because getting
    // them backwards is the mistake the page exists to prevent:
    //   catalogs  GLOBAL — one list, shared by every config version
    //   policy    PER CONFIG VERSION
    if (route === "GET /admin/billing/catalogs") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      return json(200, state.catalogs, revisionToken(version));
    }
    if (route === "POST /admin/billing/catalogs") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const mismatch = requireDraftAndMatch(version, headers);
      if (mismatch !== undefined) return mismatch;
      const body = JSON.parse(bodyText ?? "{}") as CatalogRow;
      if (state.catalogs.some((row) => row.catalog_version_id === body.catalog_version_id)) {
        return errorResponse(409, "management_lifecycle_conflict", "catalog id already exists");
      }
      // Global on purpose: no version key anywhere.
      state.catalogs.push({ ...body, created_at_ms: 1787100000000 });
      version.revision += 1;
      return json(
        201,
        {
          catalog_version_id: body.catalog_version_id,
          effective_at_ms: body.effective_at_ms,
          source: body.source,
          entry_count: body.entries.length,
          operation: "imported",
          rolled_back_from: null,
        },
        revisionToken(version),
      );
    }
    const catalogRollback = /^POST \/admin\/billing\/catalogs\/([^/]+)\/rollback$/u.exec(route);
    if (catalogRollback !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const mismatch = requireDraftAndMatch(version, headers);
      if (mismatch !== undefined) return mismatch;
      const from = decodeURIComponent(catalogRollback[1] ?? "");
      const source = state.catalogs.find((row) => row.catalog_version_id === from);
      if (source === undefined) {
        return errorResponse(404, "management_resource_not_found", "no such catalog");
      }
      const body = JSON.parse(bodyText ?? "{}") as {
        new_catalog_version_id: string;
        effective_at_ms: number;
      };
      // Forward-only: a copy is appended, nothing is deleted.
      state.catalogs.push({
        catalog_version_id: body.new_catalog_version_id,
        effective_at_ms: body.effective_at_ms,
        created_at_ms: 1787100000000,
        source: source.source,
        entries: source.entries,
      });
      version.revision += 1;
      return json(
        201,
        {
          catalog_version_id: body.new_catalog_version_id,
          effective_at_ms: body.effective_at_ms,
          source: source.source,
          entry_count: source.entries.length,
          operation: "rolled_back",
          rolled_back_from: from,
        },
        revisionToken(version),
      );
    }
    if (route === "GET /admin/billing/routing-price-policy") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const policy = state.pricePolicy.get(version.id);
      if (policy === undefined) {
        // NOT an error: an unset policy is a legitimate state, and it is the
        // reason every candidate's price_evidence reads `disabled`.
        return errorResponse(404, "management_resource_not_found", "no routing price policy");
      }
      return json(200, policy, revisionToken(version));
    }
    if (route === "PUT /admin/billing/routing-price-policy") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const mismatch = requireDraftAndMatch(version, headers);
      if (mismatch !== undefined) return mismatch;
      const body = JSON.parse(bodyText ?? "{}") as { catalog_version_id: string };
      const catalog = state.catalogs.find(
        (row) => row.catalog_version_id === body.catalog_version_id,
      );
      if (catalog === undefined) {
        return errorResponse(404, "management_resource_not_found", "no such catalog");
      }
      // The backend refuses a catalog whose effective time has not arrived
      // (RoutingPriceCatalogNotEffective). Answering 200 here would let the UI
      // offer a binding the real gateway rejects.
      if (catalog.effective_at_ms > FIXTURE_NOW_MS) {
        return errorResponse(
          409,
          "routing_price_catalog_not_effective",
          "catalog is not effective yet",
        );
      }
      state.pricePolicy.set(version.id, {
        catalog_version_id: body.catalog_version_id,
        comparison: "rate_dominance_v1",
      });
      version.revision += 1;
      return json(200, state.pricePolicy.get(version.id), revisionToken(version));
    }
    if (route === "DELETE /admin/billing/routing-price-policy") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const mismatch = requireDraftAndMatch(version, headers);
      if (mismatch !== undefined) return mismatch;
      if (!state.pricePolicy.has(version.id)) {
        return errorResponse(404, "management_resource_not_found", "no routing price policy");
      }
      state.pricePolicy.delete(version.id);
      version.revision += 1;
      return new Response(null, {
        status: 204,
        headers: new Headers({ ETag: `"${revisionToken(version)}"` }),
      });
    }

    // ---- provider account pools (P13-06B/C) ----
    //
    // The scope split is reproduced exactly: the LIST accepts no version and
    // the ACTION demands one. A fixture that let the action through without a
    // version would hide the very thing the card warns about.
    if (route === "GET /admin/operations/provider-account-pools") {
      const accounts = [
        { provider: "relay-a", channel: "ep-relay-a-responses", id: "cred-relay-key",
          kind: "api_key", auth: "active", runtime: "available", enabled: true, leases: 2 },
        { provider: "relay-a", channel: "ep-relay-a-responses", id: "cred-relay-spare",
          kind: "api_key", auth: "active", runtime: "cooling", enabled: true, leases: 0 },
        { provider: "grok-build-pool", channel: "ep-grok-build", id: "cred-grok-oauth",
          kind: "oauth", auth: "reauth_required", runtime: "unauthorized", enabled: true, leases: 0 },
        { provider: "grok-build-pool", channel: "ep-grok-build", id: "cred-grok-old",
          kind: "oauth", auth: "expired", runtime: "expired", enabled: false, leases: 0 },
        { provider: "relay-a", channel: "ep-relay-a-responses", id: "cred-relay-quota",
          kind: "api_key", auth: "active", runtime: "quota_blocked", enabled: true, leases: 0 },
      ];
      const items = accounts
        .filter((row) => {
          const p = url.searchParams.get("provider_id");
          const c = url.searchParams.get("channel_id");
          const a = url.searchParams.get("auth_status");
          const r = url.searchParams.get("runtime_status");
          const e = url.searchParams.get("enabled");
          return (
            (p === null || row.provider === p) &&
            (c === null || row.channel === c) &&
            (a === null || row.auth === a) &&
            (r === null || row.runtime === r) &&
            (e === null || String(row.enabled) === e)
          );
        })
        .map((row, index) => ({
          provider_id: row.provider,
          channel_id: row.channel,
          account_id: row.id,
          account_kind: row.kind,
          auth_status: row.auth,
          runtime_status: row.runtime,
          enabled: row.enabled,
          priority: index,
          weight: 1,
          max_concurrency: 4,
          active_leases: row.leases,
          // Nullable on purpose: an unreported due time is neither "now" nor
          // "never", and the UI has to render that difference.
          expires_at_ms: row.auth === "expired" ? FIXTURE_NOW_MS - 3_600_000 : null,
          refresh_due_at_ms: row.kind === "oauth" ? FIXTURE_NOW_MS + 7_200_000 : null,
          quota_sync_due_at_ms: null,
        }));
      return json(200, {
        snapshot_id: `snap-${state.poolSnapshot}`,
        observed_at_ms: FIXTURE_NOW_MS,
        items,
        next_cursor: null,
      });
    }
    if (route === "POST /admin/operations/provider-account-pools/actions") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const body = JSON.parse(bodyText ?? "{}") as {
        account_id: string;
        action: string;
        cooldown_ms?: number;
      };
      // A stale target is a 409, and the card must re-read rather than retry
      // blind. `cred-grok-old` stands in for "the snapshot moved under you".
      if (body.account_id === "cred-grok-old") {
        state.poolSnapshot += 1;
        return errorResponse(409, "management_lifecycle_conflict", "stale action target");
      }
      const cooling = body.action === "cool_down";
      // reauth_required cannot be probed back to life — the scheduler says so
      // rather than pretending to queue something.
      const rejected = body.account_id === "cred-grok-oauth" && !cooling;
      state.poolSnapshot += 1;
      return json(202, {
        state: cooling ? "cooling" : rejected ? "recovery_required" : "probe_scheduled",
        observed_at_ms: FIXTURE_NOW_MS,
        cooldown_until_ms: cooling ? FIXTURE_NOW_MS + (body.cooldown_ms ?? 60_000) : null,
      });
    }

    // ---- P13-11E4: provider egress status, three independent domains ----
    //
    // The three domains are served SEPARATELY here because the page asks for
    // them separately (`domain=`). Two deliberate shapes:
    //
    //   - `clearance` is EMPTY, and that is the point. The real projection's
    //     source only covers assembled Grok Build/Console runtime state, so a
    //     production deployment can truthfully report no clearance rows. The
    //     panel must say "该来源不存在" and must not read it as healthy.
    //   - `session` carries 250 rows so paging is real, and the THIRD page
    //     conflicts: that is a runtime snapshot rotating under an opaque
    //     cursor, which is the one recovery the contract requires
    //     (re-read from the start, never retry the stale cursor).
    //
    // config_conflict is bound to the older active version: the snapshot's
    // source is the draft being rolled out, so `v-2026-07` is "not this
    // snapshot's source" — the contract's own wording for that 409.
    if (route === "GET /admin/operations/provider-egress-status") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      if (version.id !== "draft-2026-08") {
        return errorResponse(
          409,
          "management_provider_egress_status_config_conflict",
          "selected config version is not this snapshot's source",
        );
      }
      const cursor = url.searchParams.get("cursor");
      if (cursor === "page-2") {
        return errorResponse(
          409,
          "management_provider_egress_status_cursor_conflict",
          "provider egress-status cursor is stale",
        );
      }
      const limit = Number(url.searchParams.get("limit") ?? "50");
      const offset = cursor === "page-1" ? limit : 0;
      const domain = url.searchParams.get("domain");

      const egressRows = [
        {
          domain: "egress",
          provider_id: "relay-a",
          upstream_id: "relay-a",
          channel_id: "ep-relay-a-responses",
          channel_kind: "generic_compatible",
          target_kind: "direct",
          target_id: null,
          state: "available",
          deadline_ms: null,
        },
        {
          domain: "egress",
          provider_id: "grok-build-pool",
          upstream_id: "grok-build-pool",
          channel_id: "ep-grok-build",
          channel_kind: "grok_build",
          target_kind: "named",
          target_id: "egress-pool-eu",
          state: "cooling_down",
          deadline_ms: FIXTURE_NOW_MS + 120_000,
        },
        {
          domain: "egress",
          provider_id: "grok-build-pool",
          upstream_id: "grok-build-pool",
          channel_id: "ep-grok-console",
          channel_kind: "grok_console",
          // Named target with no id reported: representable by the contract
          // (target_kind and target_id are independently nullable) and NOT the
          // same thing as direct. A blank cell here would erase the difference.
          target_kind: "named",
          target_id: null,
          state: "circuit_open",
          deadline_ms: FIXTURE_NOW_MS + 900_000,
        },
        {
          domain: "egress",
          provider_id: "relay-a",
          upstream_id: "relay-a",
          channel_id: "ep-relay-a-chat",
          channel_kind: "other_compatible",
          target_kind: "named",
          target_id: "egress-pool-us",
          state: "probe_due",
          deadline_ms: null,
        },
        {
          domain: "egress",
          provider_id: "relay-a",
          upstream_id: "relay-a",
          channel_id: "ep-relay-a-legacy",
          channel_kind: "generic_compatible",
          target_kind: "direct",
          target_id: null,
          state: "disabled",
          deadline_ms: null,
        },
      ];

      const sessionStates = ["active", "challenge_required", "invalid", "absent", "expired"];
      const sessionRows = Array.from({ length: 250 }, (_, index) => ({
        domain: "session",
        provider_id: index % 2 === 0 ? "relay-a" : "grok-build-pool",
        upstream_id: index % 2 === 0 ? "relay-a" : "grok-build-pool",
        channel_id: `ep-session-${index}`,
        channel_kind: index % 2 === 0 ? "generic_compatible" : "grok_build",
        // The two credentials that actually exist in this version, so the id
        // button opens a real sheet rather than a 409.
        credential_id: index % 2 === 0 ? "cred-relay-key" : "cred-grok-oauth",
        credential_revision: 2,
        session_revision: index + 1,
        state: sessionStates[index % sessionStates.length],
        expires_at_ms: index % 3 === 0 ? null : FIXTURE_NOW_MS + 3_600_000,
      }));

      const all =
        domain === "egress"
          ? egressRows
          : domain === "session"
            ? sessionRows
            : domain === "clearance"
              ? []
              : [...egressRows, ...sessionRows];
      const slice = all.slice(offset, offset + limit);
      const more = offset + slice.length < all.length;
      return json(200, {
        config_version_id: version.id,
        config_revision: version.revision,
        runtime_revision: 41,
        snapshot_id: `egress-snap-${domain ?? "all"}-7`,
        sampled_at_ms: FIXTURE_NOW_MS,
        items: slice,
        next_cursor: more ? (offset === 0 ? "page-1" : "page-2") : null,
      });
    }


    // ---- P13-11 A–D: compatible proxy pools / nodes / egress bindings ----
    //
    // The fixture enforces the three refusals the real backend enforces, so the
    // panel's client-side predictions are checked against something that
    // disagrees when they are wrong:
    //   - deleting a referenced pool or node is a conflict (there is no cascade);
    //   - (target_kind, target_id) must be one of the three admitted pairs;
    //   - proxy_endpoint must be a bare socks5://host:port.
    // And the one preservation: PATCH without proxy_endpoint keeps the sealed
    // one, which is the OPPOSITE of CredentialInput.secret.
    if (route === "GET /admin/compatible-proxy-pools") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      return json(200, state.compatPools.get(version.id) ?? [], revisionToken(version));
    }
    if (route === "GET /admin/compatible-proxy-nodes") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      return json(200, state.compatNodes.get(version.id) ?? [], revisionToken(version));
    }
    if (route === "GET /admin/compatible-egress-bindings") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      return json(200, state.compatBindings.get(version.id) ?? [], revisionToken(version));
    }
    if (route === "POST /admin/compatible-proxy-pools") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const rejected = requireDraftAndMatch(version, headers);
      if (rejected !== undefined) return rejected;
      const row = JSON.parse(bodyText ?? "{}") as CompatPoolRow;
      state.compatPools.set(version.id, [...(state.compatPools.get(version.id) ?? []), row]);
      version.revision += 1;
      return json(201, row, revisionToken(version));
    }
    if (route === "POST /admin/compatible-proxy-nodes") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const rejected = requireDraftAndMatch(version, headers);
      if (rejected !== undefined) return rejected;
      const input = JSON.parse(bodyText ?? "{}") as CompatNodeRow & { proxy_endpoint?: string };
      if (!validSocks5(input.proxy_endpoint)) {
        return errorResponse(400, "management_invalid_input", "invalid proxy endpoint");
      }
      const { proxy_endpoint: _sealed, ...rest } = input;
      const row: CompatNodeRow = { ...rest, pool_id: rest.pool_id ?? null, proxy_configured: true };
      state.compatNodes.set(version.id, [...(state.compatNodes.get(version.id) ?? []), row]);
      version.revision += 1;
      return json(201, row, revisionToken(version));
    }
    if (route === "POST /admin/compatible-egress-bindings") {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const rejected = requireDraftAndMatch(version, headers);
      if (rejected !== undefined) return rejected;
      const row = JSON.parse(bodyText ?? "{}") as CompatBindingRow;
      if (!validCompatTarget(row.target_kind, row.target_id)) {
        return errorResponse(400, "management_invalid_input", "invalid target pair");
      }
      state.compatBindings.set(version.id, [...(state.compatBindings.get(version.id) ?? []), row]);
      version.revision += 1;
      return json(201, row, revisionToken(version));
    }
    const compatPoolItem = /^(PATCH|DELETE) \/admin\/compatible-proxy-pools\/([^/]+)$/u.exec(route);
    if (compatPoolItem !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const rejected = requireDraftAndMatch(version, headers);
      if (rejected !== undefined) return rejected;
      const list = state.compatPools.get(version.id) ?? [];
      const id = decodeURIComponent(compatPoolItem[2] ?? "");
      const index = list.findIndex((row) => row.id === id);
      if (index === -1) {
        return errorResponse(409, "management_lifecycle_conflict", "unknown proxy pool");
      }
      if (compatPoolItem[1] === "DELETE") {
        const held =
          (state.compatNodes.get(version.id) ?? []).some((node) => node.pool_id === id) ||
          (state.compatBindings.get(version.id) ?? []).some(
            (b) => b.target_kind === "proxy_pool" && b.target_id === id,
          );
        if (held) {
          return errorResponse(409, "management_lifecycle_conflict", "proxy pool is referenced");
        }
        list.splice(index, 1);
        version.revision += 1;
        return new Response(null, {
          status: 204,
          headers: new Headers({ ETag: `"${revisionToken(version)}"` }),
        });
      }
      const next = { ...JSON.parse(bodyText ?? "{}"), id } as CompatPoolRow;
      list[index] = next;
      version.revision += 1;
      return json(200, next, revisionToken(version));
    }
    const compatNodeItem = /^(PATCH|DELETE) \/admin\/compatible-proxy-nodes\/([^/]+)$/u.exec(route);
    if (compatNodeItem !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const rejected = requireDraftAndMatch(version, headers);
      if (rejected !== undefined) return rejected;
      const list = state.compatNodes.get(version.id) ?? [];
      const id = decodeURIComponent(compatNodeItem[2] ?? "");
      const index = list.findIndex((row) => row.id === id);
      const current = list[index];
      if (current === undefined) {
        return errorResponse(409, "management_lifecycle_conflict", "unknown proxy node");
      }
      if (compatNodeItem[1] === "DELETE") {
        const held = (state.compatBindings.get(version.id) ?? []).some(
          (b) => b.target_kind === "fixed_proxy" && b.target_id === id,
        );
        if (held) {
          return errorResponse(409, "management_lifecycle_conflict", "proxy node is referenced");
        }
        list.splice(index, 1);
        version.revision += 1;
        return new Response(null, {
          status: 204,
          headers: new Headers({ ETag: `"${revisionToken(version)}"` }),
        });
      }
      const input = JSON.parse(bodyText ?? "{}") as CompatNodeRow & {
        proxy_endpoint?: string | null;
      };
      // Omitted or null PRESERVES the sealed endpoint; a string rotates it.
      if (
        input.proxy_endpoint !== undefined &&
        input.proxy_endpoint !== null &&
        !validSocks5(input.proxy_endpoint)
      ) {
        return errorResponse(400, "management_invalid_input", "invalid proxy endpoint");
      }
      const { proxy_endpoint: _rotated, ...rest } = input;
      list[index] = {
        ...rest,
        id,
        pool_id: rest.pool_id ?? null,
        proxy_configured: current.proxy_configured,
      };
      version.revision += 1;
      return json(200, list[index], revisionToken(version));
    }
    const compatBindingItem =
      /^(PATCH|DELETE) \/admin\/compatible-egress-bindings\/([^/]+)\/([^/]+)$/u.exec(route);
    if (compatBindingItem !== null) {
      const version = versionByHeader(headers);
      if (version instanceof Response) return version;
      const rejected = requireDraftAndMatch(version, headers);
      if (rejected !== undefined) return rejected;
      const list = state.compatBindings.get(version.id) ?? [];
      const endpointId = decodeURIComponent(compatBindingItem[2] ?? "");
      const credentialId = decodeURIComponent(compatBindingItem[3] ?? "");
      const index = list.findIndex(
        (row) => row.endpoint_id === endpointId && row.credential_id === credentialId,
      );
      if (index === -1) {
        return errorResponse(409, "management_lifecycle_conflict", "unknown binding");
      }
      if (compatBindingItem[1] === "DELETE") {
        list.splice(index, 1);
        version.revision += 1;
        return new Response(null, {
          status: 204,
          headers: new Headers({ ETag: `"${revisionToken(version)}"` }),
        });
      }
      const next = {
        ...JSON.parse(bodyText ?? "{}"),
        endpoint_id: endpointId,
        credential_id: credentialId,
      } as CompatBindingRow;
      if (!validCompatTarget(next.target_kind, next.target_id)) {
        return errorResponse(400, "management_invalid_input", "invalid target pair");
      }
      list[index] = next;
      version.revision += 1;
      return json(200, next, revisionToken(version));
    }

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
