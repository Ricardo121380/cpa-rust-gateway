// Usage and billing. Usage aggregation (GET /admin/operations/usage) carries per-metric confidence and
// provenance and is always unpriced; money comes only from the billing ledger (GET /admin/operations/billing),
// priced by versioned catalogs. Nothing is estimated from token counts.
import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { DownloadCloud, Info } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { get, send, useGet, type S } from "@/api/hooks";
import { ApiError, describeError } from "@/api/errors";
import { toast } from "@/app/toast";
import { Conf, Count, KV, Mono, PageHeader, Panel, Q, Status, Tonal } from "@/components/kit";
import { HBars, StackBar } from "@/components/charts";
import { DataTable, FilterSelect, RANGES, useParam, useRange, type Col } from "@/components/table";
import { fmt, sum } from "@/lib/format";
import { categoryOfKind, PROTOCOL, slotOf } from "@/lib/labels";
import { setParam } from "@/app/router";

type U = S<"OperationalUsageItem">;
type Agg = { id: string; req: number; input: number; output: number; reasoning: number; cache: number; conf: string; unknownReq: number };
const DIMS: [keyof U, string][] = [["public_model", "模型"], ["provider_id", "上游"], ["account_id", "账号"], ["client_key_id", "Client Key"], ["protocol", "协议"]];

function aggregate(items: U[], key: keyof U): Agg[] {
  const m = new Map<string, Agg & { confs: Set<string> }>();
  for (const u of items) {
    const id = String(u[key] ?? "—");
    const a = m.get(id) ?? { id, req: 0, input: 0, output: 0, reasoning: 0, cache: 0, conf: "exact", unknownReq: 0, confs: new Set<string>() };
    a.req += u.request_count; a.input += u.input_tokens.total ?? 0; a.output += u.output_tokens.total ?? 0; a.reasoning += u.reasoning_tokens.total ?? 0; a.cache += u.cache_read_tokens.total ?? 0;
    a.confs.add(u.input_tokens.confidence); if (u.input_tokens.confidence === "unknown") a.unknownReq += u.request_count;
    m.set(id, a);
  }
  return [...m.values()].map(({ confs, ...a }) => ({ ...a, conf: confs.has("unknown") ? (confs.size > 1 ? "partial" : "unknown") : confs.has("partial") ? "partial" : "exact" })).sort((a, b) => b.req - a.req);
}

export function Usage() {
  const range = useRange("7d");
  const tab = useParam("tab", "usage");
  const [dim, setDim] = useState<keyof U>("public_model");
  const usage = useGet<S<"OperationalUsagePage"> & { truncated?: boolean }>(["usage", range.key], "/admin/operations/usage", { query: { from_ms: range.from_ms, to_ms: range.to_ms, allow_partial: true } }, { graph: false, all: true });
  const status = useParam("cost");
  const billing = useGet<S<"OperationalBillingPage">>(["billing", range.key, status], "/admin/operations/billing", { query: { from_ms: range.from_ms, to_ms: range.to_ms, status: status || undefined, limit: 100 } }, { graph: false });
  const processing = useGet<S<"BillingProcessingStatus">>(["billing-processing"], "/admin/operations/billing-processing", undefined, { graph: false });
  const items = usage.data?.items ?? [];
  const rows = useMemo(() => aggregate(items, dim), [items, dim]);
  const T = useMemo(() => ({ input: sum(items, (u) => u.input_tokens.total), output: sum(items, (u) => u.output_tokens.total), reasoning: sum(items, (u) => u.reasoning_tokens.total), cache: sum(items, (u) => u.cache_read_tokens.total), creation: sum(items, (u) => u.cache_creation_tokens.total), req: sum(items, (u) => u.request_count), unknown: sum(items.filter((u) => u.input_tokens.confidence === "unknown"), (u) => u.request_count) }), [items]);
  const mx = rows[0]?.req || 1;

  const cols: Col<Agg>[] = [
    { key: "id", head: DIMS.find((d) => d[0] === dim)![1], cell: (r) => dim === "protocol" ? PROTOCOL[r.id] ?? r.id : <Mono className="font-semibold">{r.id}</Mono>, sort: (r) => r.id },
    { key: "req", head: "请求", align: "right", sort: (r) => r.req, cell: (r) => <div className="relative ml-auto flex min-w-[120px] justify-end"><i className="absolute left-0 top-1/2 h-1.5 -translate-y-1/2 rounded-full" style={{ width: `${(r.req / mx) * 100}%`, background: "color-mix(in srgb, var(--c1) 35%, transparent)" }} /><b className="relative">{fmt.int(r.req)}</b></div> },
    { key: "in", head: "输入", align: "right", sort: (r) => r.input, cell: (r) => fmt.n(r.input) },
    { key: "out", head: "输出", align: "right", sort: (r) => r.output, cell: (r) => fmt.n(r.output) },
    { key: "rs", head: "推理", align: "right", sort: (r) => r.reasoning, cell: (r) => fmt.n(r.reasoning) },
    { key: "cache", head: "缓存读", align: "right", sort: (r) => r.cache, cell: (r) => fmt.n(r.cache) },
    { key: "conf", head: "置信度", cell: (r) => <span className="inline-flex items-center gap-1.5"><Conf c={r.conf} />{r.unknownReq > 0 && <span className="text-[11px] text-muted-foreground">{fmt.int(r.unknownReq)} 请求无 usage</span>}</span> },
  ];
  const bcols: Col<S<"OperationalBillingItem">>[] = [
    { key: "t", head: "时间", cell: (b) => <Mono className="text-muted-foreground">{fmt.date(b.occurred_at_ms)}</Mono>, sort: (b) => b.occurred_at_ms },
    { key: "req", head: "请求", cell: (b) => <button className="font-mono text-xs underline decoration-border underline-offset-2 hover:decoration-foreground" onClick={(e) => { e.stopPropagation(); location.hash = `#/requests?range=7d&id=${b.request_id}`; }}>{b.request_id}</button> },
    { key: "model", head: "模型", cell: (b) => <Mono>{b.model}</Mono>, sort: (b) => b.model },
    { key: "acc", head: "账号", cell: (b) => <Mono className="text-xs">{b.account_id}</Mono> },
    { key: "tok", head: "输入 / 输出", align: "right", cell: (b) => `${fmt.n(b.input_tokens)} / ${fmt.n(b.output_tokens)}`, sort: (b) => b.input_tokens },
    { key: "prov", head: "usage 来源", cell: (b) => <Status map="provenance" k={b.usage_provenance ?? "unknown"} /> },
    { key: "cat", head: "价格目录", cell: (b) => (b.catalog_version_id ? <Mono className="text-xs">{b.catalog_version_id}</Mono> : <span className="text-xs text-muted-foreground">无</span>) },
    { key: "cost", head: "成本", align: "right", sort: (b) => b.cost_microunits, cell: (b) => <span className="inline-flex items-center gap-2">{fmt.money(b.cost_microunits)}<Conf c={b.cost_confidence} prefix="cost_confidence" /></span> },
  ];
  const bs = billing.data?.summary;

  return (
    <>
      <PageHeader title="用量与账单" desc="用量来自请求事件的有界聚合，每个指标标注置信度和来源；金额只来自按价格目录结算的账单记录。" actions={<Tabs value={range.key} onValueChange={(v) => setParam("range", v)}><TabsList>{RANGES.map(([k, l]) => <TabsTrigger key={k} value={k}>{l}</TabsTrigger>)}</TabsList></Tabs>} />
      <Tabs value={tab} onValueChange={(v) => setParam("tab", v)}><TabsList><TabsTrigger value="usage">用量</TabsTrigger><TabsTrigger value="billing">账单明细</TabsTrigger><TabsTrigger value="pricing">价格目录</TabsTrigger></TabsList></Tabs>

      {tab === "usage" && (<>
        <div className="grid gap-4 xl:grid-cols-[minmax(0,1.3fr)_minmax(0,1fr)]">
          <Panel title="Token 构成" desc={usage.data ? `${fmt.int(T.req)} 个请求；${fmt.int(T.unknown)} 个没有上游 usage，未计入` : undefined}>
            <Q q={usage} what="用量聚合">{() => {
              const parts = [{ label: "缓存读", v: T.cache, color: "var(--c3)" }, { label: "输入（未缓存）", v: Math.max(0, T.input - T.cache), color: "var(--c1)" }, { label: "输出", v: T.output, color: "var(--c2)" }, { label: "推理", v: T.reasoning, color: "var(--c4)" }];
              const tot = sum(parts, (p) => p.v) || 1;
              return <><StackBar parts={parts} /><div className="mt-4 grid grid-cols-2 gap-x-6 gap-y-2.5 text-sm">{parts.map((p) => <div key={p.label} className="flex items-baseline gap-2"><span className="flex flex-1 items-center gap-1.5 text-muted-foreground"><i className="size-2.5 rounded-[3px]" style={{ background: p.color }} />{p.label}</span><b className="tabular-nums">{fmt.n(p.v)}</b><span className="w-12 text-right text-xs text-muted-foreground">{fmt.pct(p.v / tot)}</span></div>)}</div>
                <p className="mt-4 flex gap-2 rounded-xl bg-muted p-3 text-xs text-muted-foreground"><Info className="mt-0.5 size-3.5 shrink-0" />输入按上游口径计（input_accounting）。缓存写 {fmt.n(T.creation)}；被排除的用量事件 {fmt.int(usage.data?.excluded_usage_events)} 条、请求组 {fmt.int(usage.data?.excluded_request_groups)} 个。</p></>;
            }}</Q>
          </Panel>
          <Panel title="按上游的请求" desc="颜色表示账号类别">
            <Q q={usage} what="用量聚合">{() => <HBars items={aggregate(items, "provider_id").map((a) => ({ key: a.id, label: <Mono>{a.id}</Mono>, v: a.req, color: `var(--c${slotOf(categoryOfKind(a.id))})` }))} />}</Q>
          </Panel>
        </div>
        <Panel flush title="明细聚合" desc={[usage.data?.observed_through_ms && `观测至 ${fmt.date(usage.data.observed_through_ms)}`, usage.data?.truncated && `只读取了前 ${fmt.int(usage.data.items.length)} 条，缩短时间范围可以看到完整数据`].filter(Boolean).join("；") || undefined} action={<Tabs value={String(dim)} onValueChange={(v) => setDim(v as keyof U)}><TabsList>{DIMS.map(([k, l]) => <TabsTrigger key={String(k)} value={String(k)}>{l}</TabsTrigger>)}</TabsList></Tabs>}>
          <Q q={usage} what="用量聚合">{() => <DataTable rows={rows} cols={cols} rowKey={(r) => r.id} initialSort={["req", "desc"]} />}</Q>
        </Panel>
      </>)}

      {tab === "billing" && (<>
        <div className="grid grid-cols-2 gap-px overflow-hidden rounded-2xl border bg-border md:grid-cols-3 xl:grid-cols-6">
          {[["已知成本", fmt.money(bs?.known_cost_microunits)], ["账单记录", fmt.int(bs?.records)], ["精确", fmt.int(bs?.exact_records)], ["部分", fmt.int(bs?.partial_records)], ["未知", fmt.int(bs?.unknown_records)], ["未定价", fmt.int(bs?.unpriced_records)]].map(([a, b]) => <div key={a} className="bg-card px-4 py-3"><div className="text-xs text-muted-foreground">{a}</div><b className="text-lg tabular-nums">{b}</b></div>)}
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <FilterSelect param="cost" label="成本置信度" options={[["exact", "精确"], ["partial", "部分"], ["unknown", "未知"], ["unpriced", "未定价"]]} />
          <Q q={processing} what="账单处理状态">{(p) => <span className="text-xs text-muted-foreground">账单处理 <Tonal tone={p.state === "current" ? "ok" : p.state === "catching_up" || p.state === "starting" ? "warn" : p.state === "disabled" ? "off" : "bad"}>{p.state}</Tonal>{p.checkpoint_ordinal != null && ` · 检查点 ${p.checkpoint_ordinal}/${p.source_ordinal}`}{p.unresolved_failures ? ` · ${p.unresolved_failures} 个未解决失败` : ""}</span>}</Q>
        </div>
        <Panel flush className="py-2"><Q q={billing} what="账单">{(d) => <DataTable rows={d.items} cols={bcols} rowKey={(b) => String(b.ledger_id)} empty="这个时间范围内没有账单记录" />}</Q></Panel>
      </>)}

      {tab === "pricing" && <Pricing />}
    </>
  );
}

function Pricing() {
  const qc = useQueryClient();
  const catalogs = useGet<S<"BillingCatalog">[]>(["billing-catalogs"], "/admin/billing/catalogs", undefined, { graph: false });
  // 404 is the contract's "no policy set", not a failure.
  const policy = useQuery({ queryKey: ["routing-price-policy"], queryFn: () => get<S<"RoutingPricePolicy">>("/admin/billing/routing-price-policy").catch((e) => { if (e instanceof ApiError && e.status === 404) return null; throw e; }) });
  const [sel, setSel] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const list = catalogs.data ?? [];
  const current = list.find((c) => c.catalog_version_id === sel) ?? list[0];
  async function refresh() {
    setBusy(true);
    try { const r = await send<Record<string, unknown>>("POST", "/admin/billing/price-source/refresh", { body: {} }); toast({ title: "已读取公开价格", description: `来源 models.dev · 不会修改任何价格目录 · ${JSON.stringify(r).slice(0, 80)}`, status: "info", duration: 6000 }); }
    catch (e) { const d = describeError(e); toast({ title: d.title, description: d.detail, status: "error" }); } finally { setBusy(false); qc.invalidateQueries({ queryKey: ["billing-catalogs"] }); }
  }
  const ecols: Col<S<"BillingCatalogEntry">>[] = [
    { key: "m", head: "模型", cell: (e) => <Mono className="font-semibold">{e.model}</Mono>, sort: (e) => e.model },
    { key: "p", head: "上游 / 端点", cell: (e) => <Mono className="text-xs">{e.provider_id} / {e.channel_id}</Mono> },
    ...(["input", "output", "reasoning", "cache_read", "cache_creation"] as const).map((k) => ({ key: k, head: { input: "输入", output: "输出", reasoning: "推理", cache_read: "缓存读", cache_creation: "缓存写" }[k], align: "right" as const, sort: (e: S<"BillingCatalogEntry">) => e[`${k}_microunits_per_million`], cell: (e: S<"BillingCatalogEntry">) => fmt.money(e[`${k}_microunits_per_million`]) })),
  ];
  return (
    <div className="grid gap-4 xl:grid-cols-[300px_minmax(0,1fr)]">
      <div className="grid content-start gap-4">
        <Panel title="路由价格策略" desc="按价格目录比较候选，不改变优先级分层">
          <Q q={policy} what="路由价格策略">{(p) => p ? <KV rows={[["目录", <Mono>{p.catalog_version_id}</Mono>], ["比较方式", <Mono>{p.comparison}</Mono>]]} /> : <p className="text-sm text-muted-foreground">未设置。路由不参考价格。</p>}</Q>
        </Panel>
        <Panel title="价格目录" desc="单位：每百万 Token 的美元" action={<Button size="sm" variant="outline" disabled={busy} onClick={refresh}><DownloadCloud />{busy ? "读取中…" : "读取公开价格"}</Button>}>
          <Q q={catalogs} what="价格目录">{() => <ul className="grid gap-1">{list.map((c) => (
            <li key={c.catalog_version_id}><button onClick={() => setSel(c.catalog_version_id)} className={`w-full rounded-xl border p-2.5 text-left transition-colors hover:bg-muted ${current?.catalog_version_id === c.catalog_version_id ? "border-border bg-card shadow-sm" : "border-transparent"}`}>
              <div className="flex items-center gap-2"><Mono className="font-semibold">{c.catalog_version_id}</Mono><Badge variant="outline" className="text-[10px]">{c.source}</Badge><Count n={c.entries.length} /></div>
              <div className="mt-0.5 text-xs text-muted-foreground">生效于 {fmt.date(c.effective_at_ms)}</div>
            </button></li>))}</ul>}</Q>
        </Panel>
      </div>
      <Panel flush title={current ? <Mono>{current.catalog_version_id}</Mono> : "目录条目"} desc="账单按请求发生时生效的目录结算；目录回滚会产生新的目录版本">
        {current ? <DataTable rows={current.entries} cols={ecols} rowKey={(e) => e.provider_id + e.channel_id + e.model} empty="这个目录没有条目" /> : <p className="px-6 py-8 text-center text-sm text-muted-foreground">还没有价格目录。导入目录后，账单才会有精确成本。</p>}
      </Panel>
    </div>
  );
}
