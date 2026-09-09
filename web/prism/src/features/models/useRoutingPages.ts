import { useInfiniteQuery } from "@tanstack/react-query";
import { call } from "../../api/client";
import { useVersionStore } from "../config-versions/versionStore";
import type { RoutingPage } from "./model";

export function useRoutingPages<T>(
  operation: "listRoutes" | "listRouteCandidates" | "listModelAliases",
  enabled = true,
) {
  const scope = useVersionStore((state) => state.context?.configVersionId);
  return useInfiniteQuery({
    queryKey: ["routing-inventory", scope, operation],
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
