// React Query bindings for the management API. Query keys include the configuration version the read
// targets, so switching between the active graph and a draft never shows the other graph's cached data.
import { keepPreviousData, useMutation, useQuery, useQueryClient, type QueryKey } from "@tanstack/react-query";
import type { components, paths } from "./schema";
import { api, unwrap } from "./client";
import { store, useConfigContext } from "./session";
import { toast } from "@/app/toast";
import { describeError, isConflict } from "./errors";

export type S<K extends keyof components["schemas"]> = components["schemas"][K];
type GetPath = { [P in keyof paths]: paths[P] extends { get: object } ? P : never }[keyof paths];

/** GET with path/query params; X-Config-Version is attached by the client middleware from the context. */
export function get<T>(path: GetPath, params?: { path?: Record<string, string>; query?: Record<string, unknown> }) {
  const query = params?.query ? Object.fromEntries(Object.entries(params.query).filter(([, v]) => v !== undefined && v !== null && v !== "")) : undefined;
  return unwrap<T>(api.GET(path as never, { params: { ...params, query } } as never) as never);
}
export function send<T>(method: "POST" | "PUT" | "PATCH" | "DELETE", path: keyof paths, opts?: { path?: Record<string, string>; query?: Record<string, unknown>; body?: unknown; headers?: Record<string, string> }) {
  const call = { POST: api.POST, PUT: api.PUT, PATCH: api.PATCH, DELETE: api.DELETE }[method] as unknown as (p: string, init: unknown) => Promise<never>;
  return unwrap<T>(call(path as string, { params: { path: opts?.path, query: opts?.query, header: opts?.headers }, body: opts?.body }));
}

export function useVersionKey() { const c = useConfigContext(); return c.working ?? c.active ?? "none"; }

export function useGet<T>(key: QueryKey, path: GetPath, params?: Parameters<typeof get>[1], opts?: { enabled?: boolean; refetchInterval?: number; graph?: boolean }) {
  const v = useVersionKey();
  return useQuery<T>({
    queryKey: opts?.graph === false ? key : [...key, v],
    queryFn: () => get<T>(path, params),
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
      if (isConflict(e)) await refreshConfigContext();
    },
  });
}

/* ---------------- configuration version context ---------------- */
export async function refreshConfigContext() {
  const list = await get<S<"ConfigVersion">[]>("/admin/config-versions");
  const active = list.find((v) => v.status === "active") ?? null;
  const { config } = store.get();
  const drafts = list.filter((v) => v.status === "draft").sort((a, b) => b.created_at_ms - a.created_at_ms);
  const keep = config.working ? list.find((v) => v.id === config.working && v.status !== "archived") : undefined;
  const working = keep ?? drafts[0] ?? active;
  store.setConfig({ active: active?.id ?? null, working: working?.id ?? null, workingStatus: working?.status ?? null, revision: working?.revision ?? null });
  return list;
}
export function useConfigVersions() {
  return useQuery({ queryKey: ["config-versions"], queryFn: refreshConfigContext });
}
export function selectWorking(v: S<"ConfigVersion">) {
  store.setConfig({ working: v.id, workingStatus: v.status, revision: v.revision });
}
export async function forkDraft(from: string, description: string) {
  const id = `draft-${new Date().toISOString().replace(/[-:TZ.]/g, "").slice(0, 14)}`;
  const v = await send<S<"ConfigVersion">>("POST", "/admin/config-versions/{config_version_id}/fork", { path: { config_version_id: from }, body: { id, description } });
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
