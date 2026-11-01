// Chart building blocks. Single y-axis everywhere; categorical colors follow identity, status colors are reserved.
import type { ReactNode } from "react";
import { Bar, BarChart, CartesianGrid, Line, LineChart, XAxis, YAxis } from "recharts";
import { ChartContainer, ChartTooltip, ChartTooltipContent, type ChartConfig } from "@/components/ui/chart";
import { fmt, sum } from "@/lib/format";

type Bucket = { at_ms: number; requests: number; succeeded: number; failed: number; cancelled?: number; average_duration_ms?: number | null; average_first_content_ms?: number | null };
const reqCfg = { succeeded: { label: "成功", color: "var(--c1)" }, failed: { label: "失败", color: "var(--bad)" } } satisfies ChartConfig;
const latCfg = { average_duration_ms: { label: "平均耗时", color: "var(--c4)" }, average_first_content_ms: { label: "平均首字", color: "var(--c3)" } } satisfies ChartConfig;

const tickFor = (series: Bucket[]) => {
  const span = series.length > 1 ? series[series.length - 1].at_ms - series[0].at_ms : 0;
  return span > 2 * 864e5 ? (t: number) => fmt.day(t) : (t: number) => new Date(t).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", hour12: false });
};

export function RequestBars({ series, height = 220 }: { series: Bucket[]; height?: number }) {
  const data = series.map((s) => ({ ...s, label: fmt.date(s.at_ms) }));
  return (
    <ChartContainer config={reqCfg} className="w-full" style={{ height }}>
      <BarChart data={data} barCategoryGap={1}>
        <CartesianGrid vertical={false} />
        <XAxis dataKey="at_ms" tickFormatter={tickFor(series)} tickLine={false} axisLine={false} minTickGap={40} />
        <YAxis tickFormatter={fmt.n} tickLine={false} axisLine={false} width={40} />
        <ChartTooltip content={<ChartTooltipContent labelKey="label" />} />
        <Bar isAnimationActive={false} dataKey="succeeded" stackId="a" fill="var(--color-succeeded)" />
        <Bar isAnimationActive={false} dataKey="failed" stackId="a" fill="var(--color-failed)" radius={[3, 3, 0, 0]} />
      </BarChart>
    </ChartContainer>
  );
}
export function LatencyLines({ series, height = 200 }: { series: Bucket[]; height?: number }) {
  const data = series.map((s) => ({ ...s, label: fmt.date(s.at_ms) }));
  return (
    <ChartContainer config={latCfg} className="w-full" style={{ height }}>
      <LineChart data={data}>
        <CartesianGrid vertical={false} />
        <XAxis dataKey="at_ms" tickFormatter={tickFor(series)} tickLine={false} axisLine={false} minTickGap={40} />
        <YAxis tickFormatter={fmt.ms} tickLine={false} axisLine={false} width={46} />
        <ChartTooltip content={<ChartTooltipContent labelKey="label" formatter={(v, n) => <span className="flex w-full justify-between gap-3"><span className="text-muted-foreground">{latCfg[n as keyof typeof latCfg]?.label}</span><b>{fmt.ms(Number(v))}</b></span>} />} />
        <Line isAnimationActive={false} dataKey="average_duration_ms" stroke="var(--color-average_duration_ms)" dot={false} strokeWidth={2} connectNulls />
        <Line isAnimationActive={false} dataKey="average_first_content_ms" stroke="var(--color-average_first_content_ms)" dot={false} strokeWidth={2} connectNulls />
      </LineChart>
    </ChartContainer>
  );
}

/** Sequential single-hue grid (Keeper "activity" squares). */
export function ActivityGrid({ cells, cols, hue = "var(--c1)", tip }: { cells: { v: number; key: string | number }[]; cols: number; hue?: string; tip: (i: number) => string }) {
  const mx = Math.max(1, ...cells.map((c) => c.v));
  return (
    <div className="hgrid" style={{ ["--cols" as string]: cols }} role="img" aria-label="活跃度方格">
      {cells.map((c, i) => <i key={c.key} title={tip(i)} className={c.v === 0 ? "" : `lv${Math.min(4, Math.ceil((c.v / mx) * 4))}`} style={{ ["--h" as string]: hue }} />)}
    </div>
  );
}
/** Success-rate strip: each cell colored by its bucket's success rate; empty buckets stay neutral. */
export function HealthGrid({ series, cols }: { series: Bucket[]; cols: number }) {
  return (
    <div className="hgrid" style={{ ["--cols" as string]: cols }} role="img" aria-label="请求健康时间线">
      {series.map((s) => {
        const r = s.requests ? 1 - s.failed / s.requests : null;
        const cls = r == null ? "" : r >= 0.995 ? "h5" : r >= 0.985 ? "h4" : r >= 0.97 ? "h3" : r >= 0.94 ? "h2" : "h1";
        return <i key={s.at_ms} className={cls} title={`${fmt.date(s.at_ms)}\n请求 ${fmt.int(s.requests)} · 失败 ${s.failed}\n成功率 ${r == null ? "—" : fmt.pct(r, 2)}`} />;
      })}
    </div>
  );
}
export function RampLegend({ from, to, colors }: { from: string; to: string; colors: string[] }) {
  return <div className="mt-3 flex items-center justify-center gap-1 text-xs text-muted-foreground"><span className="mr-1">{from}</span>{colors.map((c, i) => <i key={i} className="size-3 rounded-[3px]" style={{ background: c }} />)}<span className="ml-1">{to}</span></div>;
}
export function StackBar({ parts }: { parts: { label: string; v: number; color: string }[] }) {
  const t = sum(parts, (p) => p.v) || 1;
  return <div className="flex h-3.5 gap-0.5 overflow-hidden rounded-full" role="img" aria-label={parts.map((p) => `${p.label} ${fmt.pct(p.v / t)}`).join("，")}>{parts.filter((p) => p.v > 0).map((p) => <i key={p.label} title={`${p.label}: ${fmt.n(p.v)}（${fmt.pct(p.v / t)}）`} style={{ flex: p.v / t, background: p.color, minWidth: 3 }} />)}</div>;
}
export function HBars({ items, valFmt = fmt.n }: { items: { key: string; label: ReactNode; v: number; color: string }[]; valFmt?: (v: number) => string }) {
  const mx = Math.max(1, ...items.map((i) => i.v));
  return (
    <ol className="grid gap-3">
      {items.map((i) => (
        <li key={i.key} className="grid grid-cols-[minmax(90px,170px)_1fr_64px] items-center gap-3 text-[13px]">
          <span className="truncate">{i.label}</span>
          <span className="h-2 overflow-hidden rounded-full bg-muted"><i className="block h-full rounded-full" style={{ width: `${(i.v / mx) * 100}%`, background: i.color }} /></span>
          <b className="text-right tabular-nums">{valFmt(i.v)}</b>
        </li>
      ))}
    </ol>
  );
}
