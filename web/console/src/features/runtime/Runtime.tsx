// Runtime health: endpoint × credential availability matrix (GET /admin/runtime/availability), controlled
// recovery probes (POST /admin/runtime/quota/reset on the live graph), model catalog freshness, provider
// egress status and the bounded Prometheus exposition.
import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Play, RefreshCw } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { getText, runtimeHeaders, send, useGet, type S } from "@/api/hooks";
import { describeError } from "@/api/errors";
import { useConfigContext } from "@/api/session";
import { toast } from "@/app/toast";
import { Count, Empty, KV, Mono, PageHeader, Panel, Q, Status, toneColor } from "@/components/kit";
import { fmt } from "@/lib/format";
import { GLYPH, label, LABELS } from "@/lib/labels";

type Avail = S<"RuntimeAvailability">[number];
const WHY: Record<string, string> = {
  cooldown: "429 退避中，按 EWMA 自动恢复", quota_blocked: "配额窗口耗尽，等待窗口重置或请求恢复探测", circuit_open: "连续失败熔断，半开探测会自动进行",
  credential_forbidden: "上游拒绝（403），需要检查账号状态", credential_unauthorized: "上游未授权（401），需要重新授权", recovery_required: "需要一次受控恢复探测后才能重新入池", expired: "凭据已过期，需要重新授权",
};

export function Runtime() {
  const qc = useQueryClient();
  const ctx = useConfigContext();
  const availability = useGet<S<"RuntimeAvailability">>(["availability"], "/admin/runtime/availability", undefined, { graph: false, refetchInterval: 20_000 });
  const catalog = useGet<S<"CatalogStatus">>(["catalog"], "/admin/catalog/status", undefined, { graph: false });
  const egress = useGet<S<"ProviderEgressStatusPage">>(["egress-status"], "/admin/operations/provider-egress-status", { query: { limit: 50 } }, { graph: false });
  const metrics = useQuery({ queryKey: ["metrics"], queryFn: () => getText("/admin/observability/metrics") });
  const [target, setTarget] = useState<Avail | null>(null);
  const list = availability.data ?? [];
  const endpoints = useMemo(() => [...new Set(list.map((a) => a.endpoint_id))].sort(), [list]);
  const creds = useMemo(() => [...new Set(list.map((a) => a.credential_id))].sort(), [list]);
  const need = list.filter((a) => a.availability !== "available");

  return (
    <>
      <PageHeader title="运行时" desc={<>运行时状态作用于生效版本 <Mono>{ctx.active}</Mono>。恢复探测是受控动作：只调度一次探测，成功后才重新入池，不会直接清除冷却或配额。</>} actions={<Button variant="outline" onClick={() => qc.invalidateQueries({ queryKey: ["availability"] })}><RefreshCw />刷新</Button>} />
      <Panel title="可用性矩阵" desc="行是端点，列是凭据；只有存在绑定的格子有状态。点击受限的格子请求恢复。">
        <Q q={availability} what="运行时可用性">{() => !list.length ? <Empty title="生效版本里还没有端点 × 凭据绑定" body="绑定建立并发布后，每个组合的调度状态会显示在这里。" /> : (
          <div className="overflow-x-auto">
            <table className="border-separate border-spacing-1" aria-label="可用性矩阵">
              <thead><tr><th />{creds.map((c) => <th key={c} scope="col" className="h-24 align-bottom"><span className="inline-block rotate-180 font-mono text-[11px] font-medium text-muted-foreground [writing-mode:vertical-rl]">{c}</span></th>)}</tr></thead>
              <tbody>{endpoints.map((e) => (
                <tr key={e}><th scope="row" className="whitespace-nowrap pr-3 text-left"><Mono className="text-xs text-muted-foreground">{e}</Mono></th>
                  {creds.map((c) => {
                    const a = list.find((x) => x.endpoint_id === e && x.credential_id === c);
                    if (!a) return <td key={c}><i className="block size-7 rounded-lg opacity-60 [background:repeating-linear-gradient(45deg,transparent_0_4px,var(--muted)_4px_5px)]" /></td>;
                    const [l, tone] = label("availability", a.availability);
                    return <td key={c}><button title={`${e} × ${c}\n${l}（${a.availability}）`} aria-label={`${e} × ${c}：${l}`} disabled={a.availability === "available"} onClick={() => setTarget(a)} className="grid size-7 place-items-center rounded-lg text-xs font-bold transition-transform enabled:hover:scale-110" style={{ color: toneColor(tone), background: `color-mix(in srgb, ${toneColor(tone)} 18%, transparent)` }}>{GLYPH[tone]}</button></td>;
                  })}
                </tr>
              ))}</tbody>
            </table>
            <div className="mt-4 flex flex-wrap gap-x-5 gap-y-2">{Object.keys(LABELS.availability).map((k) => <span key={k} className="inline-flex items-center gap-1.5"><Status map="availability" k={k} /><Mono className="text-[11px] text-muted-foreground">{k}</Mono></span>)}</div>
          </div>
        )}</Q>
      </Panel>
      <div className="grid gap-4 xl:grid-cols-2">
        <Panel title={<>待处置 <Count n={need.length} /></>} desc="冷却和熔断会自愈；被拒、未授权和待恢复需要人工处理">
          {need.length ? <ul className="divide-y">{need.map((a) => (
            <li key={a.endpoint_id + a.credential_id} className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 py-3">
              <div className="min-w-0"><div className="flex flex-wrap items-center gap-2"><Mono className="font-semibold">{a.credential_id}</Mono><Status map="availability" k={a.availability} /></div><p className="mt-0.5 text-xs text-muted-foreground"><Mono>{a.endpoint_id}</Mono> · {WHY[a.availability]}</p></div>
              <Button size="sm" variant="outline" onClick={() => setTarget(a)}><Play />请求恢复</Button>
            </li>))}</ul> : <p className="py-6 text-center text-sm text-muted-foreground">所有目标都可调度</p>}
        </Panel>
        <Panel flush title="模型目录新鲜度" desc="每个端点 × 凭据最近一次读取上游模型目录的结果">
          <Q q={catalog} what="模型目录状态">{(cs) => !cs.length ? <p className="px-6 py-8 text-center text-sm text-muted-foreground">还没有读取过上游模型目录</p> : (
            <Table><TableHeader><TableRow><TableHead className="pl-6">目标</TableHead><TableHead>新鲜度</TableHead><TableHead className="text-right">模型数</TableHead><TableHead className="pr-6 text-right">最近成功</TableHead></TableRow></TableHeader>
              <TableBody>{cs.map((c) => <TableRow key={c.endpoint_id + c.credential_id}><TableCell className="pl-6"><Mono className="text-xs">{c.endpoint_id}</Mono><div className="text-xs text-muted-foreground"><Mono>{c.credential_id}</Mono></div></TableCell><TableCell><Status map="freshness" k={c.freshness} />{c.observation_state === "unsupported" && <div className="text-[11px] text-muted-foreground">端点没有 models_path</div>}{c.refresh_due && <Badge variant="outline" className="ml-1 h-4 px-1 text-[10px]">待刷新</Badge>}</TableCell><TableCell className="text-right tabular-nums">{c.model_count}</TableCell><TableCell className="pr-6 text-right text-xs text-muted-foreground">{fmt.ago(c.last_success_at_ms)}</TableCell></TableRow>)}</TableBody></Table>
          )}</Q>
        </Panel>
      </div>
      <div className="grid gap-4 xl:grid-cols-2">
        <Panel title="出口状态" desc="Provider 出口会话与清理状态"><Q q={egress} what="出口状态">{(p) => <KV rows={[["快照", <Mono>{p.snapshot_id}</Mono>], ["采样于", fmt.date(p.sampled_at_ms)], ["条目", p.items.length]]} />}</Q></Panel>
        <Panel title="Prometheus 指标" desc={<><code>GET /admin/observability/metrics</code> · 只有后台计数器，不含请求或目标标签</>}><Q q={metrics} what="指标">{(m) => <pre className="max-h-64 overflow-auto rounded-xl bg-muted p-4 font-mono text-xs leading-relaxed">{m}</pre>}</Q></Panel>
      </div>
      {target && <RecoverDialog a={target} onClose={() => setTarget(null)} />}
    </>
  );
}

function RecoverDialog({ a, onClose }: { a: Avail; onClose: () => void }) {
  const qc = useQueryClient();
  const [busy, setBusy] = useState(false);
  async function go() {
    setBusy(true);
    try {
      const r = await send<S<"RuntimeAction">>("POST", "/admin/runtime/quota/reset", { body: { endpoint_id: a.endpoint_id, credential_id: a.credential_id, upstream_model: null }, headers: runtimeHeaders() });
      toast({ title: r.state === "probe_scheduled" ? "已调度恢复探测" : r.state === "released" ? "目标已释放" : r.state === "rejected" ? "恢复请求被拒绝" : "仍需恢复", description: `RuntimeAction: ${r.state}`, status: r.state === "rejected" ? "error" : "success" });
      qc.invalidateQueries({ queryKey: ["availability"] }); onClose();
    } catch (e) { const d = describeError(e); toast({ title: d.title, description: d.detail, status: "error" }); } finally { setBusy(false); }
  }
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-[460px]">
        <DialogHeader><DialogTitle>请求恢复探测</DialogTitle><DialogDescription>{WHY[a.availability]}</DialogDescription></DialogHeader>
        <KV rows={[["端点", <Mono>{a.endpoint_id}</Mono>], ["凭据", <Mono>{a.credential_id}</Mono>], ["当前", <Status map="availability" k={a.availability} />]]} />
        <DialogFooter><Button variant="outline" onClick={onClose}>取消</Button><Button disabled={busy} onClick={go}><Play />{busy ? "正在调度…" : "调度探测"}</Button></DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
