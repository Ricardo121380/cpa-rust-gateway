// Overview — Ledger's language: daily-average card, gradient KPI tiles with sparklines, Keeper activity
// squares and a request-health strip, then account-pool water level, model ranking and recent failures.
import { useMemo, useState, type ReactNode } from "react";
import { Coins, Gauge, KeyRound, Layers, Percent, Zap } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { AnimatedCounter } from "@/components/ui/animated-counter";
import { Sparkline } from "@/components/charts/sparkline";
import { useGet, type S } from "@/api/hooks";
import { ActivityGrid, HBars, HealthGrid, RampLegend } from "@/components/charts";
import { CatDot, Mono, Panel, Q, Status, Tonal } from "@/components/kit";
import { fmt, sum } from "@/lib/format";
import { categoryOfKind, slotOf } from "@/lib/labels";
import { go } from "@/app/router";
import { QuotaWindows, tightest, useQuotas } from "@/features/accounts/quota";

const HOUR = 36e5, DAY = 864e5;
type Page = S<"RequestHistoryPage">;

function useWindow(span: "24h" | "7d") {
  return useMemo(() => {
    const to = Math.ceil(Date.now() / 6e4) * 6e4;
    return span === "7d" ? { from_ms: to - 7 * DAY, to_ms: to, bucket_ms: HOUR } : { from_ms: to - DAY, to_ms: to, bucket_ms: 10 * 6e4 };
  }, [span]);
}

function Kpi({ title, icon: I, k, value, decimals = 0, prefix, suffix, sub, data, children }: { title: string; icon: typeof Zap; k: string; value: number | null; decimals?: number; prefix?: string; suffix?: string; sub: ReactNode; data?: number[]; children?: ReactNode }) {
  return (
    <div className="kpi" style={{ ["--k" as string]: `var(--${k})` }}>
      <div className="flex items-center justify-between text-sm font-semibold text-muted-foreground">{title}<span className="grid size-9 place-items-center rounded-xl text-white" style={{ background: `var(--${k})`, boxShadow: `0 6px 14px -6px var(--${k})` }}><I className="size-[18px]" /></span></div>
      <div className="mb-1.5 mt-3 text-[32px] font-extrabold leading-none tracking-tight tabular-nums">{value == null ? "—" : <AnimatedCounter value={value} decimals={decimals} prefix={prefix} suffix={suffix} />}</div>
      <p className="mb-3 text-xs text-muted-foreground">{sub}</p>
      {data ? <div className="mt-auto rounded-xl border bg-muted/40 px-2 pt-2"><Sparkline data={data} label={title} variant="area" color={`var(--${k})`} height={50} /></div> : children}
    </div>
  );
}

export function Overview() {
  const [span, setSpan] = useState<"24h" | "7d">("7d");
  const week = useWindow("7d");
  const day = useWindow("24h");
  const summary7 = useGet<Page>(["requests-summary", week], "/admin/requests/summary", { query: week }, { graph: false });
  const summary24 = useGet<Page>(["requests-summary", day], "/admin/requests/summary", { query: day }, { graph: false });
  const billing = useGet<S<"OperationalBillingPage">>(["billing", "7d"], "/admin/operations/billing", { query: { from_ms: week.from_ms, to_ms: week.to_ms, limit: 1 } }, { graph: false });
  const usage = useGet<S<"OperationalUsagePage">>(["usage", "7d"], "/admin/operations/usage", { query: { from_ms: week.from_ms, to_ms: week.to_ms } }, { graph: false, all: true });
  const inventory = useGet<S<"UnifiedAccountPage">>(["inventory", "all"], "/admin/accounts/inventory", { query: { limit: 100 } });
  const availability = useGet<S<"RuntimeAvailability">>(["availability"], "/admin/runtime/availability", undefined, { graph: false });
  const failed = useGet<Page>(["requests", "failed"], "/admin/requests", { query: { from_ms: day.from_ms, to_ms: day.to_ms, outcome: "failed", limit: 6 } }, { graph: false });

  const sm = summary7.data?.summary;
  const series = summary7.data?.series ?? [];
  const tokens = useMemo(() => {
    const it = usage.data?.items ?? [];
    const t = (k: "input_tokens" | "output_tokens" | "reasoning_tokens" | "cache_read_tokens") => sum(it, (x) => x[k].total);
    const exactIn = sum(it.filter((x) => x.cache_read_tokens.confidence === "exact"), (x) => x.input_tokens.total);
    return { input: t("input_tokens"), output: t("output_tokens"), reasoning: t("reasoning_tokens"), cache: t("cache_read_tokens"), cacheRate: exactIn ? t("cache_read_tokens") / exactIn : null, unknown: sum(it.filter((x) => x.input_tokens.confidence === "unknown"), (x) => x.request_count) };
  }, [usage.data]);
  const pool = inventory.data?.items ?? [];
  const avail = availability.data ?? [];
  const schedulable = avail.filter((a) => a.availability === "available").length;

  const grid = span === "7d" ? series : summary24.data?.series ?? [];
  const gridCols = 24;
  const quotas = useQuotas(pool.filter((a) => a.status === "enabled" && a.category !== "api"));
  const tight = quotas.map((q, i) => ({ a: pool.filter((x) => x.status === "enabled" && x.category !== "api")[i], q })).filter((x) => x.q.data?.obs).sort((x, y) => (tightest(x.q.data!.obs) ?? 2) - (tightest(y.q.data!.obs) ?? 2)).slice(0, 5);
  const models = useMemo(() => {
    const m = new Map<string, { v: number; cat: string }>();
    (usage.data?.items ?? []).forEach((u) => { const x = m.get(u.public_model) ?? { v: 0, cat: categoryOfKind(u.provider_id) }; x.v += u.request_count; m.set(u.public_model, x); });
    return [...m].sort((a, b) => b[1].v - a[1].v).slice(0, 6);
  }, [usage.data]);
  const cost = billing.data?.summary;
  const spark = (f: (b: (typeof series)[number]) => number) => series.filter((_, i) => i % 3 === 0).map(f);

  return (
    <>
      <header className="mb-5 flex flex-wrap items-end justify-between gap-4">
        <div><h1 className="text-[24px] font-bold leading-tight">总览</h1><p className="mt-1.5 text-[13px] text-muted-foreground">近 7 天的请求、Token 与账号池状态；时间按本地时区显示。</p></div>
        <div className="flex gap-2"><Button variant="outline" onClick={() => go("requests")}>请求明细</Button><Button variant="outline" onClick={() => go("usage")}>用量与账单</Button></div>
      </header>

      <div className="grid gap-4 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.3fr)_minmax(0,1.3fr)]">
        <section className="kpi" style={{ ["--k" as string]: "var(--c4)" }}>
          <div className="mb-1 flex items-center justify-between"><b className="text-[15px]">日均</b><Badge variant="outline">范围 7 天</Badge></div>
          {[["日均请求", sm ? fmt.n(sm.requests / 7) : "—", "c1", Zap], ["日均 Token", fmt.n((tokens.input + tokens.output + tokens.reasoning) / 7), "c4", Layers], ["可调度目标", avail.length ? `${schedulable} / ${avail.length}` : "—", "c3", KeyRound]].map(([l, v, c, I]) => {
            const Icon = I as typeof Zap;
            return <div key={l as string} className="flex items-center gap-3 border-b py-3 last:border-0"><span className="grid size-8 place-items-center rounded-[10px]" style={{ background: `color-mix(in srgb, var(--${c}) 14%, transparent)`, color: `var(--${c})` }}><Icon className="size-4" /></span><span className="flex-1 font-semibold text-muted-foreground">{l as string}</span><b className="text-[22px] font-extrabold tabular-nums tracking-tight">{v as string}</b></div>;
          })}
        </section>
        <Kpi title="请求总数" icon={Zap} k="c1" value={sm?.requests ?? null} sub={sm ? <><span style={{ color: "var(--ok)" }}>●</span> 成功 {fmt.int(sm.succeeded)}　<span style={{ color: "var(--bad)" }}>●</span> 失败 {fmt.int(sm.failed)}　成功率 {fmt.pct(sm.success_rate, 2)}</> : "正在读取…"} data={series.length ? spark((b) => b.requests) : undefined} />
        <Kpi title="Token（百万）" icon={Layers} k="c4" value={usage.data ? (tokens.input + tokens.output + tokens.reasoning) / 1e6 : null} decimals={1} sub={`缓存读 ${fmt.n(tokens.cache)}　推理 ${fmt.n(tokens.reasoning)}`} data={series.length ? spark((b) => b.succeeded) : undefined} />
      </div>
      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <Kpi title="P95 耗时" icon={Gauge} k="c2" value={sm?.p95_duration_ms != null ? sm.p95_duration_ms / 1000 : null} decimals={2} suffix="s" sub={`P50 ${fmt.ms(sm?.p50_duration_ms)} · 平均首字 ${fmt.ms(sm?.average_first_content_ms)}`} data={series.length ? spark((b) => b.average_duration_ms ?? 0) : undefined} />
        <Kpi title="缓存命中" icon={Percent} k="c3" value={tokens.cacheRate != null ? +(tokens.cacheRate * 100).toFixed(1) : null} decimals={1} suffix="%" sub="仅统计 confidence = exact 的用量"><p className="mt-auto rounded-xl border bg-muted/40 px-3 py-2.5 text-[11.5px] leading-snug text-muted-foreground">缓存读 {fmt.n(tokens.cache)} / 输入 {fmt.n(tokens.input)}</p></Kpi>
        <Kpi title="重试率" icon={Zap} k="c6" value={sm ? +(((sm.attempts - sm.requests) / Math.max(1, sm.requests)) * 100).toFixed(1) : null} decimals={1} suffix="%" sub={sm ? `${fmt.int(sm.attempts)} 次尝试 / ${fmt.int(sm.requests)} 个请求` : ""}><p className="mt-auto rounded-xl border bg-muted/40 px-3 py-2.5 text-[11.5px] leading-snug text-muted-foreground">失败 {fmt.int(sm?.failed)} · 取消 {fmt.int(sm?.cancelled)} · 未知 {fmt.int(sm?.unknown)}</p></Kpi>
        <Kpi title="已知成本" icon={Coins} k="c5" value={cost?.known_cost_microunits != null ? cost.known_cost_microunits / 1e6 : null} decimals={2} prefix="$" sub={cost ? `精确 ${cost.exact_records} · 部分 ${cost.partial_records} · 未定价 ${cost.unpriced_records} 条` : "账单未接入"}>
          <p className="mt-auto rounded-xl border bg-muted/40 px-3 py-2.5 text-[11.5px] leading-snug text-muted-foreground">只合计能按价格目录计算的记录；未定价和未知记录不估算。</p>
        </Kpi>
      </div>

      <div className="!mt-8 flex flex-wrap items-center justify-between gap-3"><h2 className="text-[19px] font-bold">近期活动</h2><Tabs value={span} onValueChange={(v) => setSpan(v as "24h" | "7d")}><TabsList><TabsTrigger value="24h">24 小时</TabsTrigger><TabsTrigger value="7d">7 天</TabsTrigger></TabsList></Tabs></div>
      <div className="grid gap-4 xl:grid-cols-2">
        <Panel title="请求活跃度" desc={`每格 ${span === "7d" ? "1 小时" : "10 分钟"} 的请求数`} action={<div className="text-right"><span className="text-[11px] text-muted-foreground">请求</span><b className="block text-2xl font-extrabold tabular-nums" style={{ color: "var(--c1)" }}>{fmt.n(sum(grid, (b) => b.requests))}</b></div>}>
          <Q q={span === "7d" ? summary7 : summary24} what="请求历史">{() => <><ActivityGrid cells={grid.slice(-gridCols * Math.floor(grid.length / gridCols)).map((b) => ({ v: b.requests, key: b.at_ms }))} cols={gridCols} tip={(i) => { const b = grid.slice(-gridCols * Math.floor(grid.length / gridCols))[i]; return `${fmt.date(b.at_ms)}\n请求 ${fmt.int(b.requests)}`; }} /><RampLegend from="少" to="多" colors={["var(--muted)", ...[25, 48, 72].map((p) => `color-mix(in srgb, var(--c1) ${p}%, var(--muted))`), "var(--c1)"]} /></>}</Q>
        </Panel>
        <Panel title="请求健康时间线" desc="每格的成功率，悬停查看请求与失败数" action={<div className="text-right"><span className="text-[11px] text-muted-foreground">成功率</span><b className="block text-2xl font-extrabold tabular-nums" style={{ color: "var(--ok)" }}>{fmt.pct(1 - sum(grid, (b) => b.failed) / Math.max(1, sum(grid, (b) => b.requests)), 1)}</b></div>}>
          <Q q={span === "7d" ? summary7 : summary24} what="请求历史">{() => <><HealthGrid series={grid.slice(-gridCols * Math.floor(grid.length / gridCols))} cols={gridCols} /><RampLegend from="异常" to="健康" colors={["var(--bad)", "var(--serious)", "var(--warn)", "color-mix(in srgb, var(--ok) 55%, var(--warn))", "var(--ok)"]} /></>}</Q>
        </Panel>
      </div>

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1.25fr)_minmax(0,1fr)]">
        <Panel title="配额最紧张的账号" desc="按各账号剩余最少的窗口排序；数值是上游观测，不是账单" action={<Button size="sm" variant="outline" onClick={() => go("accounts")}>账号池</Button>}>
          <Q q={inventory} what="账号库存">{() => tight.length ? (
            <ul className="divide-y">{tight.map(({ a, q }) => (
              <li key={a.id} className="grid gap-3 py-3 first:pt-0 last:pb-0 sm:grid-cols-[minmax(0,0.9fr)_minmax(0,1.6fr)] sm:items-center">
                <div className="flex min-w-0 items-center gap-2.5"><CatDot category={a.category} size={30} /><div className="min-w-0"><Mono className="block truncate font-semibold">{a.id}</Mono><span className="block truncate text-xs text-muted-foreground">{a.identity.email ?? a.provider}{a.plan ? ` · ${a.plan}` : ""}</span></div></div>
                <QuotaWindows obs={q.data?.obs} error={q.data?.error} compact />
              </li>))}</ul>
          ) : <p className="py-6 text-center text-xs text-muted-foreground">{quotas.some((q) => q.isPending) ? "正在读取配额…" : "没有账号报告配额窗口"}</p>}</Q>
        </Panel>
        <Panel title="模型排行" desc="按 7 天请求数；颜色表示账号类别">
          <Q q={usage} what="用量聚合">{() => <HBars items={models.map(([m, x]) => ({ key: m, label: <Mono>{m}</Mono>, v: x.v, color: `var(--c${slotOf(x.cat)})` }))} />}</Q>
        </Panel>
      </div>

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1.25fr)_minmax(0,1fr)]">
        <Panel title="最近失败的请求" desc="近 24 小时" action={<Button size="sm" variant="outline" onClick={() => go("requests", { outcome: "failed" })}>全部失败</Button>} flush>
          <Q q={failed} what="请求历史">{(d) => d.items.length ? (
            <ul>{d.items.map((r) => (
              <li key={r.request_id}><button onClick={() => go("requests", { id: r.request_id })} className="grid w-full grid-cols-[64px_minmax(0,1fr)_auto] items-center gap-3 px-6 py-2.5 text-left hover:bg-muted/60">
                <Mono className="text-xs text-muted-foreground">{fmt.time(r.started_at_ms ?? 0)}</Mono>
                <div className="min-w-0"><Mono className="font-semibold">{r.model}</Mono> <span className="text-xs text-muted-foreground">{r.client_key_id}</span></div>
                <Tonal tone="bad">{r.error_code ?? "失败"}</Tonal>
              </button></li>))}</ul>
          ) : <p className="px-6 py-6 text-center text-xs text-muted-foreground">近 24 小时没有失败的请求</p>}</Q>
        </Panel>
        <Panel title="调度目标" desc="端点 × 凭据的可用性" action={<Button size="sm" variant="outline" onClick={() => go("runtime")}>运行时</Button>}>
          <Q q={availability} what="运行时可用性">{(d) => (
            <ul className="grid gap-2">{d.filter((a) => a.availability !== "available").slice(0, 6).map((a) => (
              <li key={a.endpoint_id + a.credential_id} className="flex items-center gap-3 text-sm"><Mono className="flex-1 truncate">{a.credential_id}</Mono><span className="truncate text-xs text-muted-foreground">{a.endpoint_id}</span><Status map="availability" k={a.availability} /></li>))}
              {d.every((a) => a.availability === "available") && <li className="py-4 text-center text-xs text-muted-foreground">所有 {d.length} 个目标都可调度</li>}
            </ul>
          )}</Q>
        </Panel>
      </div>
      {tokens.unknown > 0 && <p className="text-xs text-muted-foreground">注：{fmt.int(tokens.unknown)} 个请求的上游没有报告 usage（confidence = unknown），不计入 Token 与缓存统计，也不会被估算。</p>}
    </>
  );
}
