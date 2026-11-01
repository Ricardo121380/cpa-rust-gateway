// Upstream → Endpoint (one protocol each) → Credential binding, plus egress policies and compatible proxy
// pools/nodes. Every edit goes to the working draft with If-Match; endpoint test and model discovery
// preview are read-only probes.
import { useState } from "react";
import { FlaskConical, Globe, Lock, Play, Search, Server } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Switch } from "@/components/ui/switch";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ensureDraft, send, useDraftMutation, useGet, type S } from "@/api/hooks";
import { describeError } from "@/api/errors";
import { setParam } from "@/app/router";
import { CatDot, Count, Empty, KV, Mono, PageHeader, Panel, Q, Status, Tonal } from "@/components/kit";
import { useParam } from "@/components/table";
import { categoryOfKind, PROTOCOL } from "@/lib/labels";
import { cn } from "@/lib/utils";

export function Upstreams() {
  const tab = useParam("tab", "upstreams");
  return (
    <>
      <PageHeader title="上游与端点" desc="上游 → 端点（每个端点一种协议）→ 凭据绑定。修改写入当前草稿，发布后生效；凭据只能写入，不能读取或下载。" />
      <Tabs value={tab} onValueChange={(v) => setParam("tab", v)}><TabsList><TabsTrigger value="upstreams"><Server />上游</TabsTrigger><TabsTrigger value="egress"><Globe />出口策略</TabsTrigger><TabsTrigger value="proxies">代理池</TabsTrigger></TabsList></Tabs>
      {tab === "upstreams" && <UpstreamTree />}
      {tab === "egress" && <Egress />}
      {tab === "proxies" && <Proxies />}
    </>
  );
}

function UpstreamTree() {
  const upstreams = useGet<S<"Upstream">[]>(["upstreams"], "/admin/upstreams");
  const endpoints = useGet<S<"ManagedEndpointPage">>(["endpoints"], "/admin/endpoints");
  const sel = useParam("up");
  const list = upstreams.data ?? [];
  const up = list.find((u) => u.id === sel) ?? list[0];
  const toggle = useDraftMutation(async (u: S<"Upstream">) => { await ensureDraft(); await send("PATCH", "/admin/upstreams/{upstream_id}", { path: { upstream_id: u.id }, body: { ...u, enabled: !u.enabled } }); }, { invalidate: [["upstreams"]], success: (u) => `${u.name} 已${u.enabled ? "停用" : "启用"}（草稿）` });
  return (
    <Q q={upstreams} what="上游">{() => (
      <div className="grid gap-4 lg:grid-cols-[280px_minmax(0,1fr)]">
        <nav className="grid content-start gap-1 lg:sticky lg:top-20" aria-label="上游列表">
          {list.map((u) => {
            const eps = (endpoints.data?.items ?? []).filter((e) => e.upstream_id === u.id).length;
            return (
              <button key={u.id} onClick={() => setParam("up", u.id)} aria-current={u.id === up?.id} className={cn("flex items-center gap-3 rounded-xl border border-transparent p-2.5 text-left transition-colors hover:bg-muted", u.id === up?.id && "border-border bg-card shadow-sm hover:bg-card", !u.enabled && "opacity-60")}>
                <CatDot category={categoryOfKind(u.kind)} size={32} />
                <div className="min-w-0 flex-1"><b className="block truncate">{u.name}</b><Mono className="block truncate text-xs text-muted-foreground">{u.kind}</Mono></div>
                <Count n={eps} />
              </button>
            );
          })}
        </nav>
        {up && (
          <div className="grid content-start gap-4">
            <Panel title={<span className="flex items-center gap-2">{up.name}{!up.enabled && <Badge variant="outline">已停用</Badge>}</span>} desc={<span className="inline-flex flex-wrap items-center gap-1.5"><Mono>{up.id}</Mono>{up.tags.map((t) => <Badge key={t} variant="outline">{t}</Badge>)}</span>} action={<label className="flex items-center gap-2 text-xs text-muted-foreground">启用<Switch checked={up.enabled} disabled={toggle.isPending} onCheckedChange={() => toggle.mutate(up)} /></label>}>
              <KV rows={[["kind", <Mono>{up.kind}</Mono>], ["出口策略", up.egress_policy_id ? <button className="font-mono underline decoration-border underline-offset-2 hover:decoration-foreground" onClick={() => { setParam("tab", "egress"); setParam("eg", up.egress_policy_id ?? null); }}>{up.egress_policy_id}</button> : <span style={{ color: "var(--warn)" }}>未设置（启用的上游必须有出口策略）</span>]]} />
            </Panel>
            <Q q={endpoints} what="端点">{(d) => {
              const eps = d.items.filter((e) => e.upstream_id === up.id);
              return eps.length ? <>{eps.map((e) => <EndpointCard key={e.id} e={e} />)}</> : <Panel><Empty title="这个上游还没有端点" body="端点决定协议、地址和适配器。在 API 或 CLI 中创建后会显示在这里。" /></Panel>;
            }}</Q>
          </div>
        )}
      </div>
    )}</Q>
  );
}

function EndpointCard({ e }: { e: S<"Endpoint"> }) {
  const bindings = useGet<S<"Binding">[]>(["bindings", e.id], "/admin/endpoints/{endpoint_id}/credential-bindings", { path: { endpoint_id: e.id } });
  const [probe, setProbe] = useState<"test" | "discover" | null>(null);
  const toggle = useDraftMutation(async (b: S<"Binding">) => { await ensureDraft(); await send("POST", "/admin/endpoints/{endpoint_id}/credential-bindings", { path: { endpoint_id: e.id }, body: { credential_id: b.credential_id, enabled: !b.enabled, priority: b.priority, weight: b.weight, concurrency: b.concurrency } }); }, { invalidate: [["bindings", e.id], ["inventory"]], success: (b) => `${b.credential_id} 绑定已${b.enabled ? "停用" : "启用"}（草稿）` });
  return (
    <Panel title={<span className="flex items-center gap-2"><Mono>{e.id}</Mono>{!e.enabled && <Badge variant="outline">已停用</Badge>}</span>} desc={<>{PROTOCOL[e.api_format] ?? e.api_format} · <Mono>{e.adapter_id}</Mono></>}
      action={<div className="flex gap-1.5"><Button size="sm" variant="outline" onClick={() => setProbe("test")}><FlaskConical />测试</Button>{e.models_path && <Button size="sm" variant="outline" onClick={() => setProbe("discover")}><Search />发现模型</Button>}</div>}>
      <KV rows={[["base_url", <Mono>{e.base_url}</Mono>], ["inference_path", <Mono>{e.inference_path}</Mono>], ["models_path", e.models_path ? <Mono>{e.models_path}</Mono> : <span className="text-muted-foreground">无（不支持模型目录）</span>], ["transport", <Mono>{e.transport}</Mono>]]} />
      <h4 className="mb-1 mt-5 text-xs font-semibold text-muted-foreground">凭据绑定</h4>
      <Q q={bindings} what="凭据绑定">{(bs) => bs.length ? (
        <Table><TableHeader><TableRow><TableHead className="pl-0">凭据</TableHead><TableHead className="text-right">priority</TableHead><TableHead className="text-right">weight</TableHead><TableHead className="text-right">concurrency</TableHead><TableHead className="text-right">启用</TableHead></TableRow></TableHeader>
          <TableBody>{bs.map((b) => <TableRow key={b.credential_id}><TableCell className="pl-0"><Mono className="font-semibold">{b.credential_id}</Mono></TableCell><TableCell className="text-right tabular-nums">P{b.priority}</TableCell><TableCell className="text-right tabular-nums">{b.weight}</TableCell><TableCell className="text-right tabular-nums">{b.concurrency}</TableCell><TableCell className="text-right"><Switch checked={b.enabled} disabled={toggle.isPending} onCheckedChange={() => toggle.mutate(b)} aria-label={`${b.credential_id} 绑定`} /></TableCell></TableRow>)}</TableBody></Table>
      ) : <p className="py-3 text-xs text-muted-foreground">还没有凭据绑定到这个端点。在「接入账号」接入后到这里绑定。</p>}</Q>
      {probe && <ProbeDialog e={e} kind={probe} onClose={() => setProbe(null)} />}
    </Panel>
  );
}

function ProbeDialog({ e, kind, onClose }: { e: S<"Endpoint">; kind: "test" | "discover"; onClose: () => void }) {
  const [mode, setMode] = useState<"non_streaming" | "sse">("sse");
  const [state, setState] = useState<"idle" | "run" | "done">("idle");
  const [out, setOut] = useState<S<"EndpointTest"> | S<"CatalogDiff"> | null>(null);
  const [err, setErr] = useState<string | null>(null);
  async function run() {
    setState("run"); setErr(null);
    try {
      setOut(kind === "test" ? await send<S<"EndpointTest">>("POST", "/admin/endpoints/{endpoint_id}/test", { path: { endpoint_id: e.id }, body: { mode } }) : await send<S<"CatalogDiff">>("POST", "/admin/endpoints/{endpoint_id}/models/discover-preview", { path: { endpoint_id: e.id }, body: {} }));
      setState("done");
    } catch (x) { setErr(describeError(x).title); setState("idle"); }
  }
  const apply = useDraftMutation(async () => { await ensureDraft(); await send("POST", "/admin/endpoints/{endpoint_id}/models/discover-apply", { path: { endpoint_id: e.id }, body: {} }); }, { invalidate: [["catalog"]], success: "模型目录变更已写入草稿" });
  const test = out && "outcome" in out ? out : null;
  const diff = out && "added" in out ? out : null;
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-[480px]">
        <DialogHeader><DialogTitle>{kind === "test" ? "测试端点" : "发现模型"}</DialogTitle><DialogDescription><Mono>{e.id}</Mono> · {kind === "test" ? "发送一次最小请求，检查协议生命周期" : "预览上游模型目录与当前快照的差异，不修改配置"}</DialogDescription></DialogHeader>
        {kind === "test" && <Tabs value={mode} onValueChange={(v) => setMode(v as "sse")}><TabsList><TabsTrigger value="sse">SSE</TabsTrigger><TabsTrigger value="non_streaming">非流式</TabsTrigger></TabsList></Tabs>}
        {state === "run" && <span className="t-shimmer text-sm" data-text={kind === "test" ? "正在请求上游并等待首个事件…" : "正在读取上游模型目录…"}>{kind === "test" ? "正在请求上游并等待首个事件…" : "正在读取上游模型目录…"}</span>}
        {test && <div className="flex items-start gap-3">{test.outcome === "pass" && <span className="t-success-check" data-state="in" style={{ color: "var(--ok)" }}><svg viewBox="0 0 48 48" width="30" height="30" fill="none"><path d="M12 25l8 8 16-17" stroke="currentColor" strokeWidth="4" strokeLinecap="round" strokeLinejoin="round" /></svg></span>}<KV rows={[["outcome", <Status map="test" k={test.outcome} />], ["status_class", <Mono>{test.status_class ?? "—"}</Mono>], ["canonical_lifecycle", String(test.canonical_lifecycle ?? "—")]]} /></div>}
        {diff && <div className="grid grid-cols-3 gap-2 text-center">{([["新增", diff.added, "var(--ok)"], ["移除", diff.removed, "var(--bad)"], ["不变", diff.unchanged, "inherit"]] as const).map(([l, v, c]) => <div key={l} className="rounded-xl bg-muted p-3"><b className="block text-2xl tabular-nums" style={{ color: c }}>{v}</b><span className="text-xs text-muted-foreground">{l}</span></div>)}</div>}
        {err && <p role="alert" className="text-sm" style={{ color: "var(--bad)" }}>{err}</p>}
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>关闭</Button>
          {diff && (diff.added || diff.removed) ? <Button disabled={apply.isPending} onClick={() => apply.mutate(undefined, { onSuccess: onClose })}>应用到草稿</Button> : <Button disabled={state === "run"} onClick={run}><Play />{state === "done" ? "再运行一次" : "运行"}</Button>}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function Egress() {
  const egress = useGet<S<"EgressPolicy">[]>(["egress"], "/admin/egress-policies");
  const upstreams = useGet<S<"Upstream">[]>(["upstreams"], "/admin/upstreams");
  const focus = useParam("eg");
  return (
    <Q q={egress} what="出口策略">{(list) => (
      <div className="grid gap-4 lg:grid-cols-2">{list.map((e) => {
        const users = (upstreams.data ?? []).filter((u) => u.egress_policy_id === e.id);
        return (
          <Panel key={e.id} className={cn(focus === e.id && "ring-2 ring-ring")} title={e.name} desc={<Mono>{e.id}</Mono>} action={<Badge variant="outline">{e.redirect_mode === "deny" ? "拒绝重定向" : `重定向需复验 · 最多 ${e.max_redirects} 次`}</Badge>}>
            <div className="mb-4 flex flex-wrap gap-1.5">{e.allowed_hosts.map((h) => <span key={h} className="inline-flex items-center gap-1.5 rounded-lg bg-muted px-2 py-1 font-mono text-xs"><Lock className="size-3" />{h}</span>)}</div>
            <KV rows={[["协议", <Mono>{e.allowed_schemes.join(", ")}</Mono>], ["端口", e.allowed_ports.join(", ")], ["CIDR", e.allowed_cidrs.length ? <Mono>{e.allowed_cidrs.join(", ")}</Mono> : "不限制（按主机名解析后校验）"], ["引用的上游", users.length ? <span className="flex flex-wrap gap-3">{users.map((u) => <span key={u.id} className="inline-flex items-center gap-1.5"><CatDot category={categoryOfKind(u.kind)} size={18} />{u.name}</span>)}</span> : <span className="text-muted-foreground">没有上游引用</span>]]} />
          </Panel>
        );
      })}</div>
    )}</Q>
  );
}

function Proxies() {
  const pools = useGet<S<"CompatibleProxyPool">[]>(["proxy-pools"], "/admin/compatible-proxy-pools");
  const nodes = useGet<S<"CompatibleProxyNode">[]>(["proxy-nodes"], "/admin/compatible-proxy-nodes");
  const toggle = useDraftMutation(async (n: S<"CompatibleProxyNode">) => { await ensureDraft(); await send("PATCH", "/admin/compatible-proxy-nodes/{node_id}", { path: { node_id: n.id }, body: { id: n.id, upstream_id: n.upstream_id, pool_id: n.pool_id, name: n.name, enabled: !n.enabled, weight: n.weight, maximum_concurrency: n.maximum_concurrency } }); }, { invalidate: [["proxy-nodes"]], success: (n) => `${n.name} 已${n.enabled ? "停用" : "启用"}（草稿）` });
  return (
    <Q q={pools} what="代理池">{(ps) => ps.length ? (
      <div className="grid gap-4">{ps.map((p) => {
        const ns = (nodes.data ?? []).filter((n) => n.pool_id === p.id);
        return (
          <Panel key={p.id} flush title={<span className="flex items-center gap-2">{p.name}<Count n={ns.length} /></span>} desc={<>OpenAI 兼容上游的出口代理池 · <Mono>{p.upstream_id}</Mono></>} action={<Status map="group_status" k={p.enabled ? "active" : "disabled"} />}>
            <Table><TableHeader><TableRow><TableHead className="pl-6">节点</TableHead><TableHead>代理</TableHead><TableHead className="text-right">权重</TableHead><TableHead className="text-right">最大并发</TableHead><TableHead className="pr-6 text-right">启用</TableHead></TableRow></TableHeader>
              <TableBody>{ns.map((n) => <TableRow key={n.id}><TableCell className="pl-6"><b>{n.name}</b> <Mono className="text-xs text-muted-foreground">{n.id}</Mono></TableCell><TableCell>{n.proxy_configured ? <Tonal tone="ok">已配置</Tonal> : <Tonal tone="warn">未配置代理地址</Tonal>}</TableCell><TableCell className="text-right tabular-nums">{n.weight}</TableCell><TableCell className="text-right tabular-nums">{n.maximum_concurrency}</TableCell><TableCell className="pr-6 text-right"><Switch checked={n.enabled} disabled={toggle.isPending} onCheckedChange={() => toggle.mutate(n)} aria-label={`${n.name} 启用`} /></TableCell></TableRow>)}</TableBody></Table>
          </Panel>
        );
      })}</div>
    ) : <Panel><Empty title="没有代理池" body="代理池只用于 OpenAI 兼容上游的出口分流。代理地址只写入，不会在这里显示。" /></Panel>}</Q>
  );
}
