import { useInfiniteQuery } from "@tanstack/react-query";
import { call } from "../../api/client";
import { useVersionStore } from "../config-versions/versionStore";
import type { RoutingPage } from "./model";

export function routingInventoryKey(scope:string|undefined,revision:string|undefined,operation:"listRoutes"|"listRouteCandidates"|"listModelAliases") {
  return ["routing-inventory",scope,revision,operation] as const;
}

export function useRoutingPages<T>(
  operation: "listRoutes" | "listRouteCandidates" | "listModelAliases",
  enabled = true,
) {
  const context = useVersionStore((state) => state.context);
  const scope = context?.configVersionId;
  return useInfiniteQuery({
    queryKey: routingInventoryKey(scope,context?.revision,operation),
    initialPageParam: undefined as string | undefined,
    queryFn: ({ pageParam }) =>
      call<RoutingPage<T>>(
        operation,
        {
          query: {
            limit: 100,
            ...(pageParam === undefined ? {} : { cursor: pageParam }),
          },
        },
        { versionScoped: true },
      ),
    getNextPageParam: (page) => page.next_cursor ?? undefined,
    enabled: enabled && scope !== undefined,
    retry: false,
  });
}
