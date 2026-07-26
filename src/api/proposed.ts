// Access path for PROPOSED endpoints (G1 graph) that are not yet in the
// generated client. In fixture dev mode this calls the local fixture function
// directly (no network, C5 intact — the generated client remains the only
// real fetch path). In any other build it fails as "unavailable", which pages
// render as the contract-pending state.
//
// When G1 lands in management-v1.json: replace call sites with
// call("getConfigVersionGraph", ...) and delete this module.
import type { AppError } from "./errors";
import type { ConfigVersionGraph } from "./proposed-types";
import { readManagementKey } from "../session/sessionStore";

const CONTRACT_PENDING: AppError = {
  kind: "unavailable",
  code: "contract_pending",
  message: "G1 graph endpoint is not in the management contract yet",
  status: undefined,
};

function fixturesEnabled(): boolean {
  return import.meta.env.DEV && import.meta.env["VITE_PRISM_FIXTURES"] === "1";
}

export async function fetchProposedGraph(configVersionId: string): Promise<ConfigVersionGraph> {
  if (!fixturesEnabled()) {
    throw CONTRACT_PENDING;
  }
  const { fixtureFetch } = await import("../dev/fixtures");
  const response = await fixtureFetch(
    `/admin/config-versions/${encodeURIComponent(configVersionId)}/graph`,
    {
      method: "GET",
      headers: new Headers({ "X-Management-Key": readManagementKey() ?? "" }),
    },
  );
  if (!response.ok) {
    throw CONTRACT_PENDING;
  }
  return (await response.json()) as ConfigVersionGraph;
}

export function graphAvailable(): boolean {
  return fixturesEnabled();
}
