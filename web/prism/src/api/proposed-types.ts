// PROPOSED contract shapes for G1 / G7 / G3 — the TypeScript mirror of
// cpa-rust-gateway/docs/change-requests/CR-FE-001-shapes-g1-g3-g7.md.
//
// Status: proposal. Once the backend session lands these in
// management-v1.json, run `npm run sync-contract`, migrate consumers to the
// generated client, and DELETE this file. Fixtures implement these shapes so
// pages built against them need only mechanical adjustment at landing time.

import type { ConfigVersionSummary } from "../features/config-versions/versionStore";
import type { AccessGroupRecord, ClientKeyRecord } from "../features/access/model";
import type { EgressPolicy } from "../features/egress/model";
import type { PublicModel } from "../features/models/model";

// ---------- G1: GET /admin/config-versions/{id}/graph ----------

export type EndpointRecord = Readonly<{
  id: string;
  upstream_id: string;
  adapter_id: string;
  api_format: string; // "openai/responses" | "anthropic/messages" (open set)
  base_url: string;
  inference_path: string;
  models_path?: string | null;
  transport: "https";
  enabled: boolean;
}>;

export type CredentialRecord = Readonly<{
  id: string;
  upstream_id: string;
  kind: string;
  status: "active" | "disabled" | "revoked";
  revision: number;
  secret_present: true;
}>;

export type BindingRecord = Readonly<{
  endpoint_id: string;
  upstream_id: string;
  credential_id: string;
  enabled: boolean;
  priority: number;
  weight: number;
  concurrency: number;
}>;

export type UpstreamRecord = Readonly<{
  id: string;
  name: string;
  kind: string;
  enabled: boolean;
  tags: readonly string[];
  egress_policy_id?: string | null;
}>;

export type AliasRecord = Readonly<{ alias: string; public_model_id: string }>;

export type RouteRecord = Readonly<{
  id: string;
  public_model_id: string;
  policy: "smooth_weighted_round_robin";
  max_attempts: number;
  bootstrap_timeout_ms: number;
}>;

export type CandidateRecord = Readonly<{
  id: string;
  route_id: string;
  endpoint_id: string;
  upstream_model: string;
  credential_scope: "all_active";
  transform_mode: "passthrough" | "canonical" | "lossless_bridge";
  enabled: boolean;
  priority: number;
  weight: number;
  capability_override: Readonly<Record<string, boolean>>;
}>;

export type AccessGroupRouteRecord = Readonly<{
  access_group_id: string;
  route_id: string;
  enabled: boolean;
}>;

export type ConfigVersionGraph = Readonly<{
  config_version: ConfigVersionSummary;
  egress_policies: readonly EgressPolicy[];
  upstreams: readonly UpstreamRecord[];
  endpoints: readonly EndpointRecord[];
  credentials: readonly CredentialRecord[];
  bindings: readonly BindingRecord[];
  public_models: readonly PublicModel[];
  aliases: readonly AliasRecord[];
  routes: readonly RouteRecord[];
  candidates: readonly CandidateRecord[];
  access_groups: readonly AccessGroupRecord[];
  access_group_routes: readonly AccessGroupRouteRecord[];
  client_keys: readonly ClientKeyRecord[];
}>;

// ---------- G7: GET /admin/capabilities ----------

export const FEATURE_NAMES = [
  "endpoint_test",
  "catalog_discovery",
  "credential_oauth",
  "catalog_status",
  "runtime_availability",
  "quota_recovery",
  "request_attempts",
  "route_explain",
  "analytics",
  "dashboard_summary",
  "model_prices",
] as const;
export type FeatureName = (typeof FEATURE_NAMES)[number];

export type UnavailableReason = "rejecting_facade" | "pipeline_unwired" | "not_in_release";

export type FeatureState = Readonly<{ available: boolean; reason?: UnavailableReason }>;

export type CapabilitiesResponse = Readonly<{
  // tolerant of unknown future feature names
  features: Readonly<Record<string, FeatureState>>;
}>;

// ---------- G3: POST /admin/analytics ----------

export type AnalyticsBucket = "auto" | "hour" | "day";

export type AnalyticsFilters = Readonly<{
  public_model?: readonly string[];
  client_key_id?: readonly string[];
  credential_id?: readonly string[];
  endpoint_id?: readonly string[];
  upstream_id?: readonly string[];
  protocol?: "openai_responses" | "anthropic_messages" | null;
  status?: "all" | "success" | "failed";
  error_code?: readonly string[];
  error_scope?: readonly string[];
  stage?: readonly string[];
}>;

export type AnalyticsQuery = Readonly<{
  from_ms: number;
  to_ms: number;
  timezone: string;
  bucket: AnalyticsBucket;
  filters?: AnalyticsFilters;
  include: Readonly<{
    summary?: boolean;
    timeline?: boolean;
    ranks?: Readonly<{ by: "public_model" | "client_key" | "credential" | "endpoint"; limit: number }>;
    heatmap?: Readonly<{ metric: "requests" | "tokens" | "failure_rate" }>;
    options?: boolean;
    events?: Readonly<{ cursor: string | null; limit: number }>;
  }>;
}>;

export type TokenSummary = Readonly<{
  input?: number;
  output?: number;
  reasoning?: number;
  cache_read?: number;
  cache_creation?: number;
  cached?: number;
}>;

export type RequestEventView = Readonly<{
  request_id: string;
  occurred_at_ms: number;
  protocol: "openai_responses" | "anthropic_messages";
  public_model: string;
  streaming: boolean;
  outcome: "success" | "failed";
  error_code?: string | null;
  error_scope?: string | null;
  stage?: string | null;
  retry_decision?: string | null;
  attempt_count: number;
  latency_ms?: number | null;
  tokens?: TokenSummary | null;
  client_key_id: string;
  credential_id?: string | null;
  endpoint_id?: string | null;
}>;

export type AnalyticsResponse = Readonly<{
  range: Readonly<{ from_ms: number; to_ms: number; bucket: "hour" | "day"; bucket_count: number }>;
  summary?: Readonly<{
    requests: number;
    failures: number;
    attempts: number;
    tokens: TokenSummary;
    latency_ms: Readonly<{ avg?: number; p50?: number; p95?: number; p99?: number }>;
  }>;
  timeline?: ReadonlyArray<
    Readonly<{ bucket_start_ms: number; requests: number; failures: number; tokens_total: number; latency_p95_ms?: number | null }>
  >;
  ranks?: ReadonlyArray<
    Readonly<{ key: string; requests: number; failures: number; tokens_total: number; last_seen_ms: number }>
  >;
  heatmap?: ReadonlyArray<Readonly<{ weekday: number; hour: number; value: number }>>;
  options?: Readonly<Record<string, readonly string[]>>;
  events?: Readonly<{ items: readonly RequestEventView[]; next_cursor: string | null }>;
}>;

// ---------- G3: GET /admin/dashboard/summary ----------

export type HealthStripState = "empty" | "ok" | "warn" | "bad";

export type DashboardSummary = Readonly<{
  kpi: Readonly<{
    requests: number;
    failures: number;
    success_rate: number;
    tokens_total: number;
    latency_p95_ms?: number | null;
  }>;
  health_strip: ReadonlyArray<Readonly<{ bucket_start_ms: number; state: HealthStripState }>>;
  token_mix: TokenSummary;
  top_models: ReadonlyArray<Readonly<{ public_model: string; requests: number; tokens_total: number }>>;
  recent_failures: ReadonlyArray<
    Readonly<{ request_id: string; occurred_at_ms: number; error_code: string; error_scope: string; stage?: string | null }>
  >;
}>;
