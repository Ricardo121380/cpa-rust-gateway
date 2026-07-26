// Upstream-domain pure model: graph slicing per upstream + OAuth polling.
import type { ConfigVersionGraph } from "../../api/proposed-types";

export type UpstreamSubresources = Readonly<{
  endpoints: ConfigVersionGraph["endpoints"];
  credentials: ConfigVersionGraph["credentials"];
  bindings: ConfigVersionGraph["bindings"];
}>;

export function upstreamSubresources(
  graph: ConfigVersionGraph,
  upstreamId: string,
): UpstreamSubresources {
  return {
    endpoints: graph.endpoints.filter((endpoint) => endpoint.upstream_id === upstreamId),
    credentials: graph.credentials.filter((credential) => credential.upstream_id === upstreamId),
    bindings: graph.bindings.filter((binding) => binding.upstream_id === upstreamId),
  };
}

export type OAuthState = "pending" | "complete" | "cancelled" | "failed";

/** TanStack Query refetchInterval: poll every 2s while pending, else stop. */
export function oauthPollIntervalMs(state: OAuthState | undefined): number | false {
  return state === "pending" ? 2000 : false;
}

export function oauthStateBadge(state: OAuthState): string {
  // maps onto the shared badge vocabulary (StatusBadge tones)
  switch (state) {
    case "pending":
      return "recovery_required"; // tint tone: in progress
    case "complete":
      return "active";
    case "cancelled":
      return "disabled";
    case "failed":
      return "credential_forbidden";
  }
}
