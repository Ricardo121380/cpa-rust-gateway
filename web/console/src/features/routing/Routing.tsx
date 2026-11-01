// Public models, aliases and routes. A route holds candidates (endpoint × upstream model) in priority
// tiers; requests fall to a lower tier only when no credential in the tier above is available, and
// candidates in a tier share traffic by smooth weight. Route Explain (GET /admin/routes/{id}/explain)
// shows the live decision for each candidate.
import { useMemo, useState } from "react";
import { Check, Search, X } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ensureDraft, send, useDraftMutation, useGet, type S } from "@/api/hooks";
import { setParam } from "@/app/router";
import { CatDot, Count, KV, Mono, PageHeader, Panel, Q, Status, Tonal, toneColor } from "@/components/kit";
import { useParam } from "@/components/table";
import { fmt, sum } from "@/lib/format";
import { categoryOfKind, label, slotOf, TRANSFORM } from "@/lib/labels";
import { cn } from "@/lib/utils";

type Model = S<"PublicModel">;
type Route = S<"RouteListItem">;
type Cand = S<"Candidate">;

export function Routing() {
  const models = useGet<Model[]>(["public-models"], "/admin/public-models");
  const routes = useGet<S<"RoutePage">>(["routes"], "/admin/routes");
  const cands = useGet<S<"CandidatePage">>(["candidates"], "/admin/route-candidates");
  const aliases = useGet<S<"AliasPage">>(["aliases"], "/admin/model-aliases");
  const endpoints = useGet<S<"ManagedEndpointPage">>(["endpoints"], "/admin/endpoints");
  const upstreams = useGet<S<"Upstream">[]>(["upstreams"], "/admin/upstreams");
  const sel = useParam("model");
  const list = models.data ?? [];
  const pm = list.find((m) => m.id === sel) ?? list[0];
  const route = routes.data?.items.find((r) => r.public_model_id === pm?.id);
  const epUp = useMemo(() => new Map((endpoints.data?.items ?? []).map((e) => [e.id, (upstreams.data ?? []).find((u) => u.id === e.upstream_id)])), [endpoints.data, upstreams.data]);
  const toggleModel = useDraftMutation(async (m: Model) => { await ensureDraft(); await send("PATCH", "/admin/public-models/{public_model_id}", { path: { public_model_id: m.id }, body: { ...m, status: m.status === "active" ? "disabled" : "active" } }); }, { invalidate: [["public-models"]], success: (m) => `${m.model_name} 已${m.status === "active" ? "停用" : "启用"}（草稿）` });

  return (
    <>
      <PageHeader title="模型与路由" desc="客户端看到的是公开模型；每个公开模型对应一条路由，路由把请求分发到各端点上的上游模型。" />
      <Q q={models} what="公开模型">{() => (
        <div className="grid gap-4 lg:grid-cols-[280px_minmax(0,1fr)]">
          <nav className="grid content-start gap-1 lg:sticky lg:top-20" aria-label="公开模型">
            {list.map((m) => {
              const r = routes.data?.items.find((x) => x.public_model_id === m.id);
              const n = (cands.data?.items ?? []).filter((c) => c.route_id === r?.id).length;
              const al = (aliases.data?.items ?? []).filter((a) => a.public_model_id === m.id).map((a) => a.alias);
              return (
                <button key={m.id} onClick={() => setParam("model", m.id)} aria-current={m.id === pm?.id} className={cn("flex items-center gap-3 rounded-xl border border-transparent p-2.5 text-left transition-colors hover:bg-muted", m.id === pm?.id && "border-border bg-card shadow-sm hover:bg-card", m.status !== "active" && "opacity-60")}>
                  <div className="min-w-0 flex-1"><Mono className="block truncate font-semibold">{m.model_name}</Mono><span className="block truncate text-xs text-muted-foreground">{m.display_name}{al.length ? ` · 别名 ${al.join(", ")}` : ""}</span></div>
                  {m.status === "active" ? <Count n={n} /> : <Status map="group_status" k="disabled" />}
                </button>
              );
            })}
          </nav>
          {pm && (
            <div className="grid content-start gap-4">
              <Panel title={<Mono>{pm.model_name}</Mono>} desc={<>{pm.display_name} · <Mono>{pm.id}</Mono></>} action={<label className="flex items-center gap-2 text-xs text-muted-foreground">启用<Switch checked={pm.status === "active"} disabled={toggleModel.isPending} onCheckedChange={() => toggleModel.mutate(pm)} /></label>}>
                <div className="mb-4 flex flex-wrap gap-1.5">{Object.entries(pm.capabilities).map(([k, v]) => <Badge key={k} variant="outline" className="font-mono" style={v ? { color: "var(--ok)", borderColor: "color-mix(in srgb, var(--ok) 35%, transparent)" } : { color: "var(--muted-foreground)" }}>{v ? <Check /> : <X />}{k}</Badge>)}</div>
                <KV rows={[["路由", route ? <Mono>{route.id}</Mono> : <span style={{ color: "var(--warn)" }}>没有路由：启用的公开模型必须有路由</span>], ["调度策略", route ? <Mono>{route.policy}</Mono> : "—"], ["最多尝试", route?.max_attempts ?? "—"], ["首字节超时", route ? fmt.ms(route.bootstrap_timeout_ms) : "—"], ["别名", (aliases.data?.items ?? []).filter((a) => a.public_model_id === pm.id).map((a) => <Mono key={a.alias} className="mr-2">{a.alias}</Mono>)]]} />
              </Panel>
              {route && <RouteTiers route={route} candidates={(cands.data?.items ?? []).filter((c) => c.route_id === route.id)} epUp={epUp} model={pm.model_name} />}
            </div>
          )}
        </div>
      )}</Q>
    </>
  );
}

function RouteTiers({ route, candidates, epUp, model }: { route: Route; candidates: Cand[]; epUp: Map<string, S<"Upstream"> | undefined>; model: string }) {
  const [protocol, setProtocol] = useState<"openai_responses" | "openai_chat_completions" | "anthropic_messages">("openai_responses");
  const explain = useGet<S<"RouteExplain">>(["explain", route.id, protocol], "/admin/routes/{route_id}/explain", { path: { route_id: route.id }, query: { requested_model: model, protocol } });
  const availability = useGet<S<"RuntimeAvailability">>(["availability"], "/admin/runtime/availability", undefined, { graph: false });
  const tiers = [...new Set(candidates.map((c) => c.priority))].sort((a, b) => a - b);
  const total = (t: number) => sum(candidates.filter((c) => c.priority === t && c.enabled), (c) => c.weight) || 1;
  const toggle = useDraftMutation(async (c: Cand) => { await ensureDraft(); const { route_id, ...input } = c; await send("PATCH", "/admin/routes/{route_id}/candidates/{candidate_id}", { path: { route_id, candidate_id: c.id }, body: { ...input, enabled: !c.enabled } }); }, { invalidate: [["candidates"]], success: (c) => `候选 ${c.id} 已${c.enabled ? "停用" : "启用"}（草稿）` });
  return (
    <Panel title="分层候选" desc="上层没有可用凭据时才落到下一层；层内按权重平滑轮转。右侧为 Explain 对当前运行时的判断。" action={<Tabs value={protocol} onValueChange={(v) => setProtocol(v as typeof protocol)}><TabsList><TabsTrigger value="openai_responses">Responses</TabsTrigger><TabsTrigger value="openai_chat_completions">Chat</TabsTrigger><TabsTrigger value="anthropic_messages">Messages</TabsTrigger></TabsList></Tabs>}>
      <div className="grid gap-3 md:grid-cols-[repeat(auto-fit,minmax(240px,1fr))]">
        {tiers.map((t) => (
          <div key={t} className="flex flex-col gap-2 rounded-2xl border border-dashed p-2.5">
            <div className="flex justify-between px-1 text-xs font-semibold text-muted-foreground"><span>Tier P{t}</span><span>{t === tiers[0] ? "优先" : "回退"}</span></div>
            {candidates.filter((c) => c.priority === t).map((c) => {
              const up = epUp.get(c.endpoint_id); const cat = categoryOfKind(up?.kind);
              const x = explain.data?.candidates.find((y) => y.candidate_id === c.id);
              const creds = (availability.data ?? []).filter((a) => a.endpoint_id === c.endpoint_id);
              return (
                <div key={c.id} className={cn("grid gap-2 rounded-xl border bg-muted/40 p-3 transition-opacity", (x?.decision === "excluded" || !c.enabled) && "opacity-70")} style={x?.decision === "selected" ? { borderColor: "color-mix(in srgb, var(--ok) 45%, transparent)", background: "color-mix(in srgb, var(--ok) 6%, var(--card))" } : undefined}>
                  <div className="flex items-center justify-between gap-2"><span className="inline-flex min-w-0 items-center gap-1.5"><CatDot category={cat} size={20} /><b className="truncate text-sm">{up?.name ?? c.endpoint_id}</b></span>{x ? <Tonal tone={x.decision === "selected" ? "ok" : "off"}>{x.decision === "selected" ? "选中" : "排除"}</Tonal> : explain.isPending ? <span className="text-xs text-muted-foreground">…</span> : null}</div>
                  <div className="flex flex-wrap items-center gap-1.5 text-xs"><Mono>{c.endpoint_id}</Mono><span className="text-muted-foreground">→</span><Mono>{c.upstream_model}</Mono><Badge variant="outline" className="h-4 px-1.5 text-[10px]">{TRANSFORM[c.transform_mode]}</Badge></div>
                  <div className="flex items-center gap-2 text-[11px] text-muted-foreground"><span className="h-1.5 flex-1 overflow-hidden rounded-full bg-muted" title={`权重 ${c.weight} / ${total(t)}`}><i className="block h-full rounded-full" style={{ width: `${c.enabled ? (c.weight / total(t)) * 100 : 0}%`, background: `var(--c${slotOf(cat)})` }} /></span>权重 {c.weight}</div>
                  {creds.length > 0 && <div className="flex flex-wrap gap-1">{creds.map((a) => { const [l, tone] = label("availability", a.availability); return <i key={a.credential_id} title={`${a.credential_id} · ${l}`} className="h-1.5 w-4 rounded-sm" style={{ background: toneColor(tone) }} />; })}</div>}
                  {x?.reason && <Mono className="text-[11px] leading-snug text-muted-foreground">{x.reason}</Mono>}
                  <label className="flex items-center justify-end gap-2 text-[11px] text-muted-foreground">启用<Switch checked={c.enabled} disabled={toggle.isPending} onCheckedChange={() => toggle.mutate(c)} className="scale-90" /></label>
                </div>
              );
            })}
          </div>
        ))}
        {tiers.length === 0 && <p className="py-6 text-center text-sm text-muted-foreground">这条路由还没有候选，请求会返回 RouteNotFound。</p>}
      </div>
      {explain.isError && <p className="mt-3 text-xs text-muted-foreground"><Search className="mr-1 inline size-3" />Explain 不可用：当前协议下没有可解释的候选。</p>}
    </Panel>
  );
}

/** Compact listing used by the access page grant matrix. */
export function useRouteLabels() {
  const models = useGet<Model[]>(["public-models"], "/admin/public-models");
  const routes = useGet<S<"RoutePage">>(["routes"], "/admin/routes");
  return useMemo(() => (routes.data?.items ?? []).map((r) => ({ route: r, model: (models.data ?? []).find((m) => m.id === r.public_model_id) })), [models.data, routes.data]);
}
