import { useQuery } from "@tanstack/react-query";
import { call } from "../../api/client";
import { shouldRetryManagementRead } from "../../api/errors";
import type { BillingResponse } from "../monitoring/model";

/** The KPI and completeness card share one request and refresh policy per range. */
export function useOverviewBilling(range: Readonly<{from_ms: number; to_ms: number}>) {
  return useQuery({
    queryKey: ["overview-billing", range],
    queryFn: () => call<BillingResponse>("listOperationalBilling", {query: {...range, limit: 1}}),
    retry: shouldRetryManagementRead,
    retryDelay: attempt => 250 * (attempt + 1),
    refetchInterval: 60_000,
    staleTime: 15_000,
  });
}
