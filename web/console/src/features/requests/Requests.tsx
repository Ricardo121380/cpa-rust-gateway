// Request history (GET /admin/requests + /admin/requests/summary) with filters in the URL, cursor
// pagination, and an attempt timeline drawer (GET /admin/requests/{id}/attempts).
import { useEffect, useState } from "react";
import { useInfiniteQuery } from "@tanstack/react-query";
import { ChevronsDown, ShieldCheck } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ToolRowBase } from "@/components/agent-elements/components/tools/tool-row-base";
import { get, useGet, type S } from "@/api/hooks";
import { Conf, KV, Legend, Mono, PageHeader, Panel, Q, Status, Tonal } from "@/components/kit";
import { LatencyLines, RequestBars } from "@/components/charts";
import { ClearFilters, DataTable, FilterSearch, FilterSelect, RANGES, useParam, useRange, type Col } from "@/components/table";
import { fmt } from "@/lib/format";
import { PROTOCOL, STAGE } from "@/lib/labels";
import { setParam, useRoute } from "@/app/router";

type Row = S<"RequestHistoryRow">;
type Page = S<"RequestHistoryPage">;
const FILTERS = ["outcome", "model", "upstream_id", "credential_id", "client_key_id", "request_id"];

export function Requests() {
  const range = useRange("24h");
  const { params } = useRoute();
  const filters = Object.fromEntries(FILTERS.map((k) => [k, params.get(k) || undefined]));
  const openId = useParam("id");
  const query = { from_ms: range.from_ms, to_ms: range.to_ms, ...filters };
  const summary = useGet<Page>(["requests-summary", query, range.bucket_ms], "/admin/requests/summary", { query: { ...query, bucket_ms: range.bucket_ms } }, { graph: false });
  const list = useInfiniteQuery({
    queryKey: ["requests", query],
    initialPageParam: null as string | null,
    queryFn: ({ pageParam }) => get<Page>("/admin/requests", { query: { ...query, limit: 50, cursor: pageParam ?? undefined } }),
    getNextPageParam: (p) => p.next_cursor,
  });
  const rows = list.data?.pages.flatMap((p) => p.items) ?? [];
  const sm = summary.data?.summary;
  const usage = useGet<S<"OperationalUsagePage">>(["usage", "options"], "/admin/operations/usage", { query: { from_ms: Date.now() - 7 * 864e5, limit: 500 } }, { graph: false });
  const opt = (k: "public_model" | "provider_id" | "client_key_id" | "account_id") => [...new Set((usage.data?.items ?? []).map((u) => u[k]))].sort().map((v) => [v, v] as [string, string]);

  const cols: Col<Row>[] = [
    { key: "t", head: "开始时间", cell: (r) => <Mono className="text-muted-foreground">{r.started_at_ms ? fmt.time(r.started_at_ms) : "—"}</Mono>, sort: (r) => r.started_at_ms },
    { key: "model", head: "模型", cell: (r) => <div><Mono className="font-semibold">{r.model}</Mono>{r.requested_model !== r.model && <div className="text-[11px] text-muted-foreground">请求 {r.requested_model}</div>}</div>, sort: (r) => r.model },
    { key: "proto", head: "协议", cell: (r) => <span className="whitespace-nowrap text-xs">{PROTOCOL[r.protocol] ?? r.protocol}{r.streaming && <Badge variant="outline" className="ml-1.5 h-4 px-1 text-[10px]">SSE</Badge>}</span> },
    { key: "outcome", head: "结果", cell: (r) => <div><Status map="outcome" k={r.outcome} />{r.error_code && <div className="font-mono text-[11px] text-muted-foreground">{r.error_code}</div>}</div>, sort: (r) => r.outcome },
    { key: "att", head: "尝试", cell: (r) => <span className="inline-flex items-center gap-1" title={`${r.attempt_count} 次尝试`}>{Array.from({ length: Math.min(r.attempt_count, 5) }, (_, i) => <i key={i} className="size-2.5 rounded-[3px]" style={{ background: i < r.attempt_count - 1 ? "var(--warn)" : r.outcome === "succeeded" ? "var(--ok)" : r.outcome === "failed" ? "var(--bad)" : "var(--muted-foreground)" }} />)}</span>, sort: (r) => r.attempt_count },
    { key: "target", head: "上游 / 凭据", cell: (r) => <div className="text-xs"><Mono>{r.upstream_id ?? "—"}</Mono><div className="text-muted-foreground"><Mono>{r.credential_id ?? "—"}</Mono></div></div> },
    { key: "key", head: "Client Key", cell: (r) => <Mono className="text-xs">{r.client_key_id}</Mono> },
    { key: "first", head: "首字", align: "right", cell: (r) => fmt.ms(r.first_content_ms), sort: (r) => r.first_content_ms },
    { key: "dur", head: "耗时", align: "right", cell: (r) => fmt.ms(r.duration_ms), sort: (r) => r.duration_ms },
    { key: "tok", head: "输入 / 输出", align: "right", cell: (r) => <span className="whitespace-nowrap">{fmt.n(r.usage?.input_tokens)} / {fmt.n(r.usage?.output_tokens)}{r.usage && r.usage.provenance !== "measured" && <> <Status map="provenance" k={r.usage.provenance} /></>}</span>, sort: (r) => r.usage?.input_tokens },
    { key: "cost", head: "成本", align: "right", cell: (r) => <span className="whitespace-nowrap">{fmt.money(r.cost_microunits)} {r.cost_confidence !== "exact" && <Conf c={r.cost_confidence} prefix="cost_confidence" />}</span>, sort: (r) => r.cost_microunits },
  ];

  return (
    <>
      <PageHeader title="请求" desc="每个客户端请求一行；点击查看 Attempt 时间线。事件链路只保留阶段与分类，不记录正文、Cookie 或上游错误原文。" actions={<Tabs value={range.key} onValueChange={(v) => setParam("range", v)}><TabsList>{RANGES.map(([k, l]) => <TabsTrigger key={k} value={k}>{l}</TabsTrigger>)}</TabsList></Tabs>} />
      <div className="grid grid-cols-2 gap-px overflow-hidden rounded-2xl border bg-border sm:grid-cols-3 xl:grid-cols-6">
        {[["请求", fmt.int(sm?.requests)], ["成功率", fmt.pct(sm?.success_rate, 2)], ["失败 / 取消", sm ? `${fmt.int(sm.failed)} / ${fmt.int(sm.cancelled)}` : "—"], ["重试", sm ? fmt.int(sm.attempts - sm.requests) : "—"], ["P50 / P95 耗时", sm ? `${fmt.ms(sm.p50_duration_ms)} / ${fmt.ms(sm.p95_duration_ms)}` : "—"], ["平均首字", fmt.ms(sm?.average_first_content_ms)]].map(([a, b]) => <div key={a} className="bg-card px-4 py-3"><div className="text-xs text-muted-foreground">{a}</div><b className="text-lg tabular-nums">{b}</b></div>)}
      </div>
      <div className="grid gap-4 xl:grid-cols-2">
        <Panel title="请求量" desc={`每 ${range.bucket_ms >= 36e5 ? range.bucket_ms / 36e5 + " 小时" : range.bucket_ms / 6e4 + " 分钟"}`}><Q q={summary} what="请求历史">{(d) => <><RequestBars series={d.series} height={190} /><Legend items={[{ label: "成功", color: "var(--c1)" }, { label: "失败", color: "var(--bad)" }]} /></>}</Q></Panel>
        <Panel title="延迟" desc="同一纵轴：平均耗时与平均首字"><Q q={summary} what="请求历史">{(d) => <><LatencyLines series={d.series} height={190} /><Legend items={[{ label: "平均耗时", color: "var(--c4)" }, { label: "平均首字", color: "var(--c3)" }]} /></>}</Q></Panel>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <FilterSelect param="outcome" label="结果" options={[["succeeded", "成功"], ["failed", "失败"], ["cancelled", "已取消"], ["unknown", "未知"]]} />
        <FilterSelect param="model" label="模型" options={opt("public_model")} />
        <FilterSelect param="upstream_id" label="上游" options={opt("provider_id")} />
        <FilterSelect param="client_key_id" label="Key" options={opt("client_key_id")} />
        <FilterSearch param="request_id" placeholder="按 request_id 精确查找" />
        <ClearFilters keys={FILTERS} />
        <span className="ml-auto text-xs text-muted-foreground">已加载 {rows.length} 条</span>
      </div>
      <Panel flush className="py-2">
        {list.isError ? <Q q={summary}>{() => null}</Q> : <DataTable rows={rows} cols={cols} rowKey={(r) => r.request_id} onRow={(r) => setParam("id", r.request_id)} empty={list.isPending ? "正在加载…" : "这个时间范围内没有匹配的请求"} selected={(r) => r.request_id === openId} />}
        {list.hasNextPage && <div className="flex justify-center pt-3"><Button variant="outline" size="sm" disabled={list.isFetchingNextPage} onClick={() => list.fetchNextPage()}><ChevronsDown />{list.isFetchingNextPage ? "正在加载…" : "加载更多"}</Button></div>}
      </Panel>
      <RequestSheet id={openId} row={rows.find((r) => r.request_id === openId)} onClose={() => setParam("id", null)} />
    </>
  );
}

function RequestSheet({ id, row, onClose }: { id: string; row?: Row; onClose: () => void }) {
  const attempts = useGet<S<"Attempt">[]>(["attempts", id], "/admin/requests/{request_id}/attempts", { path: { request_id: id } }, { enabled: !!id, graph: false });
  const [open, setOpen] = useState(!!id);
  useEffect(() => setOpen(!!id), [id]);
  return (
    <Sheet open={open} onOpenChange={(o) => { setOpen(o); if (!o) onClose(); }}>
      <SheetContent className="w-[min(560px,100vw)] overflow-y-auto sm:max-w-[560px]">
        <SheetHeader><SheetTitle className="font-mono">{id}</SheetTitle><SheetDescription>{row ? `${row.started_at_ms ? fmt.date(row.started_at_ms) : ""} · ${PROTOCOL[row.protocol] ?? row.protocol}${row.streaming ? " · SSE" : ""}` : "请求详情"}</SheetDescription></SheetHeader>
        <div className="grid gap-5 px-4 pb-6">
          {row && <KV rows={[
            ["模型", <><Mono>{row.model}</Mono>{row.requested_model !== row.model && <span className="text-muted-foreground">（请求 {row.requested_model}）</span>}</>],
            ["结果", <span className="inline-flex gap-2"><Status map="outcome" k={row.outcome} />{row.error_code && <Mono className="text-xs">{row.error_code}</Mono>}</span>],
            ["Client Key", <Mono>{row.client_key_id}</Mono>],
            ["上游 / 端点", <Mono>{row.upstream_id} / {row.endpoint_id}</Mono>],
            ["首字 / 总耗时", `${fmt.ms(row.first_content_ms)} / ${fmt.ms(row.duration_ms)}`],
            ["usage", row.usage ? <span className="inline-flex flex-wrap items-center gap-2">in {fmt.n(row.usage.input_tokens)} · out {fmt.n(row.usage.output_tokens)} · 推理 {fmt.n(row.usage.reasoning_tokens)} · 缓存读 {fmt.n(row.usage.cache_read_tokens)} <Status map="provenance" k={row.usage.provenance} /></span> : "未报告"],
            ["成本", <span className="inline-flex items-center gap-2">{fmt.money(row.cost_microunits)} <Conf c={row.cost_confidence} prefix="cost_confidence" /> <span className="text-xs text-muted-foreground">{row.ledger_records} 条账单记录</span></span>],
          ]} />}
          <section>
            <h4 className="mb-2 text-xs font-semibold text-muted-foreground">Attempt 时间线</h4>
            <Q q={attempts} what="Attempt 记录">{(list) => (
              <ol className="grid gap-1.5">{list.map((a) => {
                const o = a.observation;
                const tone = a.outcome === "succeeded" ? "ok" : o?.retry_decision === "retry_eligible" ? "warn" : "bad";
                return (
                  <li key={a.attempt_id} className="rounded-xl border bg-muted/40 px-3 py-2">
                    <ToolRowBase
                      icon={<i className="block size-2.5 rounded-full" style={{ background: `var(--${tone})` }} />}
                      completeLabel={`#${o?.attempt_number ?? "?"} ${a.outcome}`}
                      isAnimating={false}
                      detail={`${a.stage ? STAGE[a.stage] ?? a.stage : "—"} · ${fmt.ms(o?.duration_ms)}`}
                      trailingContent={<Tonal tone={tone}>{o?.retry_decision ?? a.outcome}</Tonal>}
                      expandable
                      defaultOpen={tone !== "ok"}
                    >
                      <div className="mt-2 pl-6"><KV rows={[["attempt_id", <Mono>{a.attempt_id}</Mono>], ["endpoint / credential", <Mono>{a.endpoint_id} / {a.credential_id}</Mono>], ["stage", <Mono>{a.stage}</Mono>], ["error", o?.error_code ? <Mono>{o.error_code} · {o.error_scope}</Mono> : "无"], ["时间", o ? `${fmt.time(o.started_at_ms)} → ${fmt.time(o.ended_at_ms)}` : "—"]]} /></div>
                    </ToolRowBase>
                  </li>
                );
              })}</ol>
            )}</Q>
          </section>
          <p className="flex gap-2 rounded-xl bg-muted p-3 text-xs text-muted-foreground"><ShieldCheck className="size-4 shrink-0" />网关不保存请求正文和上游错误原文，这里只显示阶段、错误分类和重试决策。</p>
        </div>
      </SheetContent>
    </Sheet>
  );
}
