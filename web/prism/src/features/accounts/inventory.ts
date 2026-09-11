import type { AccountIdentity, Category } from "./presentation";
import { useInfiniteQuery } from "@tanstack/react-query";
import { call } from "../../api/client";
import { useVersionStore } from "../config-versions/versionStore";

export type ManagedCredential = Readonly<{
  credential: Readonly<{
    id: string;
    upstream_id: string;
    kind: string;
    status: "active" | "disabled" | "revoked";
    revision: number;
    secret_present: boolean;
  }>;
  binding_count: number;
  identity: AccountIdentity;
  category: Exclude<Category,"grok">;
  provider: string;
  connections: readonly {id:string; api_format:string; enabled:boolean; host:string|null}[];
}>;

export type ManagedEndpoint = Readonly<{
  id: string;
  upstream_id: string;
  adapter_id: string;
  api_format: string;
  base_url: string;
  inference_path: string;
  models_path: string | null;
  transport: string;
  enabled: boolean;
}>;

export type InventoryPage<T> = Readonly<{
  config_version: string;
  revision: string;
  observation_version: string;
  items: readonly T[];
  next_cursor: string | null;
}>;

export function useManagedInventory<K extends "credentials" | "endpoints">(
  kind: K,
  upstreamId?: string,
  search = "",
) {
  const scope = useVersionStore((state) => state.context?.configVersionId);
  type Item = K extends "credentials" ? ManagedCredential : ManagedEndpoint;
  return useInfiniteQuery({
    queryKey: ["managed-inventory", scope, kind, upstreamId, search],
    initialPageParam: undefined as string | undefined,
    enabled: scope !== undefined,
    retry: false,
    queryFn: ({ pageParam }) => call<InventoryPage<Item>>(
      kind === "credentials" ? "listManagedCredentials" : "listManagedEndpoints",
      { query: { limit: 100, ...(upstreamId ? { upstream_id: upstreamId } : {}), ...(search ? { q: search } : {}), ...(pageParam ? { cursor: pageParam } : {}) } },
      { versionScoped: true },
    ),
    getNextPageParam: (page) => page.next_cursor ?? undefined,
  });
}
