import type { PublicModel, RouteListItem } from "./model";

/** Compare declared values, including explicit false capabilities, without JSON key-order drift. */
export function samePublicModel(left: PublicModel | undefined, right: PublicModel): boolean {
  if (left === undefined) return false;
  if (left.id !== right.id || left.model_name !== right.model_name || left.display_name !== right.display_name || left.status !== right.status) return false;
  const keys = new Set([...Object.keys(left.capabilities), ...Object.keys(right.capabilities)]);
  return [...keys].every((key) => left.capabilities[key] === right.capabilities[key]);
}

export function sameRoute(left: RouteListItem | undefined, right: RouteListItem): boolean {
  return left !== undefined && left.id === right.id && left.public_model_id === right.public_model_id
    && left.policy === right.policy && left.max_attempts === right.max_attempts
    && left.bootstrap_timeout_ms === right.bootstrap_timeout_ms;
}
