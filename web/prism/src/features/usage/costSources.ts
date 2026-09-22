import type { LedgerRow } from "../monitoring/model";
import type { Filters } from "./model";

export function supportsCostFilters(filters: Filters): boolean {
  return !filters.client_key_id && !filters.access_group_id && !filters.protocol;
}

/** Exact model IDs group ledger records; a missing price never becomes zero. */
export function costSources(rows: readonly LedgerRow[]) {
  const groups = new Map<string, {model:string; records:number; known:number|null; incomplete:number}>();
  for (const row of rows) {
    const group = groups.get(row.model) ?? {model:row.model, records:0, known:null, incomplete:0};
    group.records += 1;
    if (row.cost_microunits !== null) group.known = (group.known ?? 0) + row.cost_microunits;
    if (row.cost_confidence !== "exact") group.incomplete += 1;
    groups.set(row.model, group);
  }
  return [...groups.values()].sort((a,b)=>(b.known??-1)-(a.known??-1)||a.model.localeCompare(b.model));
}
