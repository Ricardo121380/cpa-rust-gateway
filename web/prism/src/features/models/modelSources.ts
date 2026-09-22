import type { CandidateRecord, RouteListItem } from "./model";

/** Include every configured route, including disabled sources for maintenance. */
export function modelSources(modelId:string,routes:readonly RouteListItem[],candidates:readonly CandidateRecord[]) {
  const routeIds=new Set(routes.filter(route=>route.public_model_id===modelId).map(route=>route.id));
  return candidates.filter(candidate=>routeIds.has(candidate.route_id));
}
