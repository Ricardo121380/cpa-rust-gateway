// Hash routing (#/page?filters) so the embedded SPA needs no server-side fallback route,
// and filters survive in the URL for drill-down links between pages.
import { useSyncExternalStore } from "react";

export type Route = { page: string; params: URLSearchParams };

function parse(): Route {
  const raw = location.hash.replace(/^#\/?/, "");
  const [page, query = ""] = raw.split("?");
  return { page: page || "overview", params: new URLSearchParams(query) };
}
let current = parse();
const subs = new Set<() => void>();
addEventListener("hashchange", () => { current = parse(); subs.forEach((s) => s()); });

export function useRoute() {
  return useSyncExternalStore((s) => { subs.add(s); return () => subs.delete(s); }, () => current);
}
export function go(page: string, params?: Record<string, string | number | undefined | null>) {
  const q = new URLSearchParams();
  for (const [k, v] of Object.entries(params ?? {})) if (v !== undefined && v !== null && v !== "") q.set(k, String(v));
  const s = q.toString();
  location.hash = `#/${page}${s ? "?" + s : ""}`;
}
/** Replace one query param on the current page without adding a history entry. */
export function setParam(key: string, value: string | null) {
  const p = new URLSearchParams(current.params);
  if (value === null || value === "") p.delete(key); else p.set(key, value);
  const s = p.toString();
  history.replaceState(null, "", `#/${current.page}${s ? "?" + s : ""}`);
  current = parse();
  subs.forEach((f) => f());
}
