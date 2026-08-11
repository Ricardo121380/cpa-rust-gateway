// Access path for PROPOSED endpoints (G3 analytics) that are not yet in the
// generated client. In fixture dev mode this calls the local fixture function
// directly (no network, C5 intact — the generated client remains the only
// real fetch path). In any other build it fails as "unavailable", which pages
// render as the contract-pending state.
//
// The G1 graph channel is gone: P13-04A shipped listOperationalAccountPools,
// which serves the per-provider subresource case for real. Only the analytics
// channel is left, and P13-05 is expected to replace it too.
import type { AppError } from "./errors";
import type {
  AnalyticsQuery,
  AnalyticsResponse,
  DashboardSummary,
} from "./proposed-types";
import { readManagementKey } from "../session/sessionStore";

const CONTRACT_PENDING: AppError = {
  kind: "unavailable",
  code: "contract_pending",
  message: "analytics endpoint is not in the management contract yet",
  status: undefined,
};

function fixturesEnabled(): boolean {
  return import.meta.env.DEV && import.meta.env["VITE_PRISM_FIXTURES"] === "1";
}

export function analyticsAvailable(): boolean {
  return fixturesEnabled();
}

async function proposedPost<T>(path: string, body: unknown): Promise<T> {
  if (!fixturesEnabled()) {
    throw CONTRACT_PENDING;
  }
  const { fixtureFetch } = await import("../dev/fixtures");
  const response = await fixtureFetch(path, {
    method: "POST",
    headers: new Headers({
      "X-Management-Key": readManagementKey() ?? "",
      "Content-Type": "application/json",
    }),
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    throw CONTRACT_PENDING;
  }
  return (await response.json()) as T;
}

export async function fetchProposedAnalytics(query: AnalyticsQuery): Promise<AnalyticsResponse> {
  return proposedPost<AnalyticsResponse>("/admin/analytics", query);
}

export async function fetchProposedDashboard(
  todayStartMs: number,
  nowMs: number,
): Promise<DashboardSummary> {
  if (!fixturesEnabled()) {
    throw CONTRACT_PENDING;
  }
  const { fixtureFetch } = await import("../dev/fixtures");
  const response = await fixtureFetch(
    `/admin/dashboard/summary?today_start_ms=${todayStartMs}&now_ms=${nowMs}&top_models=5&recent_failures=5`,
    {
      method: "GET",
      headers: new Headers({ "X-Management-Key": readManagementKey() ?? "" }),
    },
  );
  if (!response.ok) {
    throw CONTRACT_PENDING;
  }
  return (await response.json()) as DashboardSummary;
}
