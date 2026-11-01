// React Query bindings for the management API. Query keys include the configuration version the read
// targets, so switching between the active graph and a draft never shows the other graph's cached data.
import { keepPreviousData, useMutation, useQuery, useQueryClient, type QueryKey } from "@tanstack/react-query";
import type { components, paths } from "./schema";
import { api, LIVE_READS, unwrap } from "./client";
import { store, useConfigContext } from "./session";
import { toast } from "@/app/toast";
import { ApiError, describeError, isConflict, NO_CONFIG_VERSION } from "./errors";
import { OPS } from "./ops.gen";

export type S<K extends keyof components["schemas"]> = components["schemas"][K];
type GetPath = { [P in keyof paths]: paths[P] extends { get: object } ? P : never }[keyof paths];

/** GET with path/query params; X-Config-Version is attached by the client middleware from the context. */
export async function get<T>(path: GetPath, params?: { path?: Record<string, string>; query?: Record<string, unknown> }) {
  await requireVersion(path);
  const query = params?.query ? Object.fromEntries(Object.entries(params.query).filter(([, v]) => v !== undefined && v !== null && v !== "")) : undefined;
  return unwrap<T>(api.GET(path as never, { params: { ...params, query } } as never) as never);
}
/**
 * Reads whose contract requires X-Config-Version wait for the version list; on a gateway with no version
 * they fail locally instead of sending a request the gateway can only reject with 400.
 */
async function requireVersion(path: GetPath) {
  if ((OPS as Record<string, { cv: number }>)[`GET ${path}`]?.cv !== 2) return;
  await store.whenConfigLoaded();
  const { config } = store.get();
  const target = LIVE_READS.has(path) ? config.active : config.working ?? config.active;
  if (!target) throw new ApiError(0, NO_CONFIG_VERSION, "no configuration version");
}
/** Plain-text GET (Prometheus exposition). */
export async function getText(path: GetPath) {
  await requireVersion(path);
  return unwrap<string>(api.GET(path as never, { parseAs: "text" } as never) as never);
}
export function send<T>(method: "POST" | "PUT" | "PATCH" | "DELETE", path: keyof paths, opts?: { path?: Record<string, string>; query?: Record<string, unknown>; body?: unknown; headers?: Record<string, string> }) {
  const call = { POST: api.POST, PUT: api.PUT, PATCH: api.PATCH, DELETE: api.DELETE }[method] as unknown as (p: string, init: unknown) => Promise<never>;
  return unwrap<T>(call(path as string, { params: { path: opts?.path, query: opts?.query, header: opts?.headers }, body: opts?.body }));
}

export function useVersionKey() { const c = useConfigContext(); return c.working ?? c.active ?? "none"; }

/** The contract caps list pages at 100 items. */
export const PAGE_MAX = 100;
type Paged = { items: unknown[]; next_cursor?: string | null };
/**
 * Follows `next_cursor` for aggregate views that need the whole window, at most `maxPages` pages.
 * Page-level fields come from the first page; `truncated` is true when more pages were left unread.
 */
export async function getAll<T extends Paged>(path: GetPath, params?: Parameters<typeof get>[1], maxPages = 20): Promise<T & { truncated: boolean }> {
  let first: T | undefined;
  const items: unknown[] = [];
  let cursor: string | null | undefined;
  for (let i = 0; i < maxPages; i++) {
    const page = await get<T>(path, { ...params, query: { ...params?.query, limit: PAGE_MAX, cursor: cursor ?? undefined } });
    first ??= page;
    items.push(...page.items);
    cursor = page.next_cursor;
    if (!cursor) return { ...first, items, next_cursor: null, truncated: false };
  }
  return { ...first!, items, next_cursor: cursor, truncated: true };
}

export function useGet<T>(key: QueryKey, path: GetPath, params?: Parameters<typeof get>[1], opts?: { enabled?: boolean; refetchInterval?: number; graph?: boolean; all?: boolean }) {
  const v = useVersionKey();
  return useQuery<T>({
    queryKey: opts?.graph === false ? key : [...key, v],
    queryFn: () => (opts?.all ? (getAll(path, params) as Promise<T>) : get<T>(path, params)),
    enabled: opts?.enabled ?? true,
    refetchInterval: opts?.refetchInterval,
    placeholderData: keepPreviousData,
  });
}

/**
 * Mutation against the working draft. On success it invalidates the given keys; on 409 it re-reads the
 * version (new revision) and tells the operator what happened instead of retrying silently.
 */
export function useDraftMutation<V>(fn: (v: V) => Promise<unknown>, opts: { invalidate: QueryKey[]; success?: string | ((v: V) => string) }) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onSuccess: (_d, v) => {
      opts.invalidate.forEach((k) => qc.invalidateQueries({ queryKey: k }));
      qc.invalidateQueries({ queryKey: ["config-versions"] });
      if (opts.success) toast({ title: typeof opts.success === "function" ? opts.success(v) : opts.success, status: "success" });
    },
    onError: async (e) => {
      const d = describeError(e);
      toast({ title: d.title, description: d.detail, status: "error", duration: 6000 });
      if (!isConflict(e)) return;
      // Someone else changed the graph: re-read both the revision and the resources this write was about,
      // so the operator decides on what is there now rather than on the stale row they clicked.
      await refreshConfigContext();
      opts.invalidate.forEach((k) => qc.invalidateQueries({ queryKey: k }));
      qc.invalidateQueries({ queryKey: ["config-versions"] });
    },
  });
}

/* ---------------- configuration version context ---------------- */
export async function refreshConfigContext() {
  let list: S<"ConfigVersion">[];
  // Even a failed read settles the context, so version-scoped reads report instead of waiting forever.
  try { list = await get<S<"ConfigVersion">[]>("/admin/config-versions"); } catch (e) { store.setConfig({ loaded: true }); throw e; }
  const active = list.find((v) => v.status === "active") ?? null;
  const { config } = store.get();
  const drafts = list.filter((v) => v.status === "draft").sort((a, b) => b.created_at_ms - a.created_at_ms);
  const keep = config.working ? list.find((v) => v.id === config.working && v.status !== "archived") : undefined;
  const working = keep ?? drafts[0] ?? active;
  store.setConfig({ active: active?.id ?? null, working: working?.id ?? null, workingStatus: working?.status ?? null, revision: working?.revision ?? null, loaded: true });
  return list;
}
/**
 * The contract answers validation with `Validation{valid,error_codes}`, but the gateway reports an invalid
 * graph as `409 management_lifecycle_conflict` without reasons (CR-CONSOLE-VALIDATE-RESULT-001).
 * Both shapes become a Validation here so every caller renders "not valid" rather than a request error.
 */
export async function validateVersion(id: string): Promise<S<"Validation">> {
  try { return await send<S<"Validation">>("POST", "/admin/config-versions/{config_version_id}/validate", { path: { config_version_id: id } }); }
  catch (e) { if (e instanceof ApiError && e.code === "management_lifecycle_conflict") return { valid: false, error_codes: [e.code] }; throw e; }
}
export function useConfigVersions() {
  return useQuery({ queryKey: ["config-versions"], queryFn: refreshConfigContext });
}
export function selectWorking(v: S<"ConfigVersion">) {
  store.setConfig({ working: v.id, workingStatus: v.status, revision: v.revision });
}
export async function forkDraft(from: string, description: string) {
  const id = `draft-${new Date().toISOString().replace(/[-:TZ.]/g, "").slice(0, 14)}`;
  // Fork is a write against the *source* version: its X-Config-Version and If-Match name the source.
  const src = (await get<S<"ConfigVersion">[]>("/admin/config-versions")).find((v) => v.id === from);
  if (!src) throw new Error(`找不到配置版本 ${from}`);
  const v = await send<S<"ConfigVersion">>("POST", "/admin/config-versions/{config_version_id}/fork", { path: { config_version_id: from }, body: { id, description }, headers: { "X-Config-Version": from, "If-Match": `"${src.revision.replace(/"/g, "")}"` } });
  selectWorking(v);
  return v;
}

/** Ensure writes target a draft: returns the draft id, forking from the active graph when needed. */
export async function ensureDraft() {
  const { config } = store.get();
  if (config.workingStatus === "draft" && config.working) return config.working;
  if (!config.active) throw new Error("没有生效的配置版本");
  const v = await forkDraft(config.active, "控制台编辑");
  toast({ title: `已从 ${config.active} 创建草稿 ${v.id}`, description: "后续修改都写入这个草稿，发布前不影响线上。", status: "info" });
  return v.id;
}

/** Runtime actions (recovery probes, cool-downs, quota resets) act on the live graph, not on a draft. */
export function runtimeHeaders(): Record<string, string> {
  const { config } = store.get();
  return config.active ? { "X-Config-Version": config.active } : {};
}
