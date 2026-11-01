// Data table helpers on top of shadcn Table: horizontal scroll on narrow screens, sortable headers
// with aria-sort, and a filter bar whose values live in the URL (so links and Back restore them).
import { useMemo, useState, type ReactNode } from "react";
import { ArrowDown, ArrowUp, ArrowUpDown, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { cn } from "@/lib/utils";
import { setParam, useRoute } from "@/app/router";

export type Col<T> = { key: string; head: ReactNode; cell: (r: T) => ReactNode; sort?: (r: T) => number | string | null | undefined; className?: string; align?: "right" };

export function DataTable<T>({ rows, cols, rowKey, onRow, empty, selected, dense, initialSort }: { rows: T[]; cols: Col<T>[]; rowKey: (r: T) => string; onRow?: (r: T) => void; empty?: ReactNode; selected?: (r: T) => boolean; dense?: boolean; initialSort?: [string, "asc" | "desc"] }) {
  const [sort, setSort] = useState<[string, "asc" | "desc"] | null>(initialSort ?? null);
  const sorted = useMemo(() => {
    if (!sort) return rows;
    const col = cols.find((c) => c.key === sort[0]);
    if (!col?.sort) return rows;
    const dir = sort[1] === "asc" ? 1 : -1;
    return [...rows].sort((a, b) => { const x = col.sort!(a), y = col.sort!(b); if (x == null) return 1; if (y == null) return -1; return x < y ? -dir : x > y ? dir : 0; });
  }, [rows, cols, sort]);
  return (
    <Table className={cn(dense && "[&_td]:py-1.5")}>
      <TableHeader>
        <TableRow>
          {cols.map((c, i) => {
            const on = sort?.[0] === c.key;
            return (
              <TableHead key={c.key} className={cn(i === 0 && "pl-6", i === cols.length - 1 && "pr-6", c.align === "right" && "text-right", c.className)} aria-sort={on ? (sort![1] === "asc" ? "ascending" : "descending") : c.sort ? "none" : undefined}>
                {c.sort ? (
                  <button className={cn("inline-flex items-center gap-1 hover:text-foreground", c.align === "right" && "flex-row-reverse")} onClick={() => setSort(on && sort![1] === "desc" ? [c.key, "asc"] : [c.key, "desc"])}>
                    {c.head}{on ? (sort![1] === "asc" ? <ArrowUp className="size-3" /> : <ArrowDown className="size-3" />) : <ArrowUpDown className="size-3 opacity-40" />}
                  </button>
                ) : c.head}
              </TableHead>
            );
          })}
        </TableRow>
      </TableHeader>
      <TableBody>
        {sorted.length === 0 && <TableRow><TableCell colSpan={cols.length} className="py-10 text-center text-sm text-muted-foreground">{empty ?? "没有数据"}</TableCell></TableRow>}
        {sorted.map((r) => (
          <TableRow key={rowKey(r)} data-state={selected?.(r) ? "selected" : undefined} className={cn(onRow && "cursor-pointer")} tabIndex={onRow ? 0 : undefined} onClick={onRow ? () => onRow(r) : undefined} onKeyDown={onRow ? (e) => { if (e.key === "Enter") onRow(r); } : undefined}>
            {cols.map((c, i) => <TableCell key={c.key} className={cn(i === 0 && "pl-6", i === cols.length - 1 && "pr-6", c.align === "right" && "text-right tabular-nums", c.className)}>{c.cell(r)}</TableCell>)}
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

/** Read a filter from the hash URL. */
export function useParam(key: string, fallback = "") {
  const { params } = useRoute();
  return params.get(key) ?? fallback;
}

export function FilterSelect({ param, label, options, all = "全部" }: { param: string; label: string; options: [string, string][]; all?: string }) {
  const v = useParam(param);
  return (
    <Select value={v || "__all"} onValueChange={(x) => setParam(param, x === "__all" ? null : x)}>
      <SelectTrigger size="sm" className="min-w-[132px] bg-card" aria-label={label}><span className="text-muted-foreground">{label}</span><SelectValue /></SelectTrigger>
      <SelectContent><SelectItem value="__all">{all}</SelectItem>{options.map(([k, l]) => <SelectItem key={k} value={k}>{l}</SelectItem>)}</SelectContent>
    </Select>
  );
}
export function FilterSearch({ param, placeholder }: { param: string; placeholder: string }) {
  const v = useParam(param);
  const [text, setText] = useState(v);
  return (
    <form className="relative" onSubmit={(e) => { e.preventDefault(); setParam(param, text.trim() || null); }}>
      <Input value={text} onChange={(e) => setText(e.target.value)} onBlur={() => setParam(param, text.trim() || null)} placeholder={placeholder} className="h-8 w-[220px] bg-card pr-7 text-[13px]" aria-label={placeholder} />
      {text && <button type="button" className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground" aria-label="清除" onClick={() => { setText(""); setParam(param, null); }}><X className="size-3.5" /></button>}
    </form>
  );
}
export function ClearFilters({ keys }: { keys: string[] }) {
  const { params } = useRoute();
  if (!keys.some((k) => params.get(k))) return null;
  return <Button size="sm" variant="ghost" onClick={() => keys.forEach((k) => setParam(k, null))}><X />清除筛选</Button>;
}
export const RANGES: [string, string, number][] = [["1h", "1 小时", 36e5], ["24h", "24 小时", 864e5], ["7d", "7 天", 7 * 864e5], ["30d", "30 天", 30 * 864e5]];
export function useRange(def = "24h") {
  const r = useParam("range", def);
  return useMemo(() => {
    const span = RANGES.find((x) => x[0] === r)?.[2] ?? 864e5;
    const to = Math.ceil(Date.now() / 6e4) * 6e4;
    return { key: r, from_ms: to - span, to_ms: to, bucket_ms: span <= 36e5 ? 6e4 * 2 : span <= 864e5 ? 6e5 : span <= 7 * 864e5 ? 36e5 : 6 * 36e5 };
  }, [r]);
}
