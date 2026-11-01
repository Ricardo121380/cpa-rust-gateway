// Account pool: unified inventory (GET /admin/accounts/inventory) joined with runtime pool state
// (GET /admin/operations/provider-account-pools) and live quota windows. Bulk recovery / cool-down go
// through POST /admin/operations/provider-account-pools/actions on the live graph; enable/disable is a
// configuration change and lands in the working draft.
import { useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Layers, MoreHorizontal, Plus, RefreshCw, Snowflake, X } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { ensureDraft, runtimeHeaders, send, useGet, type S } from "@/api/hooks";
import { describeError } from "@/api/errors";
import { toast } from "@/app/toast";
import { go, setParam } from "@/app/router";
import { CatDot, Count, KV, Mono, PageHeader, Panel, Q, Status } from "@/components/kit";
import { FilterSearch, useParam } from "@/components/table";
import { fmt } from "@/lib/format";
import { CATEGORY } from "@/lib/labels";
import { cn } from "@/lib/utils";
import { QuotaWindows, useQuota, useQuotas } from "./quota";

type Item = S<"UnifiedAccountItem">;
type Pool = S<"ProviderAccountPoolItem">;
const ISSUE: Record<string, (i: Item, p?: Pool) => boolean> = {
  all: () => true,
  attention: (i, p) => i.status !== "enabled" || (!!p && p.runtime_status !== "available"),
  reauth: (i, p) => i.status === "reauth_required" || p?.auth_status === "reauth_required",
  limited: (_i, p) => !!p && ["cooling", "quota_blocked", "circuit_open"].includes(p.runtime_status),
  disabled: (i) => i.status === "disabled",
};

export function Accounts() {
  const qc = useQueryClient();
  const category = useParam("category");
  const q = useParam("q");
  const [issue, setIssue] = useState("all");
  const [sel, setSel] = useState<string[]>([]);
  const open = useParam("account");
  const inventory = useGet<S<"UnifiedAccountPage">>(["inventory", category, q], "/admin/accounts/inventory", { query: { category: category || undefined, q: q || undefined, limit: 100 } });
  const pools = useGet<S<"ProviderAccountPoolPage">>(["pools"], "/admin/operations/provider-account-pools", undefined, { graph: false, refetchInterval: 30_000, all: true });
  const poolBy = useMemo(() => new Map((pools.data?.items ?? []).map((p) => [p.account_id, p])), [pools.data]);
  const items = (inventory.data?.items ?? []).filter((i) => ISSUE[issue](i, poolBy.get(i.id)));
  const quotas = useQuotas(items.filter((i) => i.category !== "api"));
  const quotaOf = (id: string) => { const idx = items.filter((i) => i.category !== "api").findIndex((i) => i.id === id); return idx >= 0 ? quotas[idx] : undefined; };
  const totals = inventory.data?.category_totals ?? {};
  const toggle = (id: string) => setSel((s) => (s.includes(id) ? s.filter((x) => x !== id) : [...s, id]));

  async function poolAction(action: "request_recovery" | "cool_down", ids: string[]) {
    let ok = 0;
    for (const id of ids) {
      const p = poolBy.get(id);
      if (!p) continue;
      try { await send("POST", "/admin/operations/provider-account-pools/actions", { body: { provider_id: p.provider_id, channel_id: p.channel_id, account_id: id, action, cooldown_ms: action === "cool_down" ? 10 * 60_000 : undefined }, headers: runtimeHeaders() }); ok++; }
      catch (e) { const d = describeError(e); toast({ title: `${id}：${d.title}`, description: d.detail, status: "error" }); }
    }
    if (ok) toast({ title: action === "request_recovery" ? `已为 ${ok} 个账号调度恢复探测` : `${ok} 个账号进入 10 分钟冷却`, description: "探测成功后才会重新入池，不会直接清除冷却", status: "success" });
    setSel([]); qc.invalidateQueries({ queryKey: ["pools"] });
  }
  async function setEnabled(ids: string[], enabled: boolean) {
    try {
      await ensureDraft();
      for (const id of ids) {
        const i = items.find((x) => x.id === id);
        if (!i) continue;
        if (i.native && i.native_account) await send("PATCH", "/admin/native-accounts/{account_id}", { path: { account_id: id }, body: { revision: i.native_account.revision, enabled } });
        else if (i.managed) await send("PATCH", "/admin/credentials/{credential_id}/status", { path: { credential_id: id }, body: { status: enabled ? "active" : "disabled", credential_revision: i.managed.credential.revision } });
      }
      toast({ title: `${ids.length} 个账号已${enabled ? "启用" : "停用"}`, description: "托管凭据的状态写入了草稿，发布后生效", status: "success" });
    } catch (e) { const d = describeError(e); toast({ title: d.title, description: d.detail, status: "error" }); }
    setSel([]); qc.invalidateQueries({ queryKey: ["inventory"] });
  }

  return (
    <>
      <PageHeader title="账号池" desc="所有渠道的账号：授权状态、调度状态与上游报告的配额窗口。配额是观测值，不作为账单或路由依据。" actions={<><Button variant="outline" onClick={() => qc.invalidateQueries({ queryKey: ["quota"] })}><RefreshCw />刷新配额</Button><Button onClick={() => go("onboarding")}><Plus />接入账号</Button></>} />
      <div className="flex gap-2 overflow-x-auto pb-1 [scrollbar-width:none]">
        {[["", "全部", inventory.data?.total], ...Object.entries(totals).map(([k, n]) => [k, CATEGORY[k] ?? k, n])].map(([k, l, n]) => (
          <button key={String(k)} onClick={() => setParam("category", (k as string) || null)} className={cn("flex h-11 shrink-0 items-center gap-2 rounded-2xl border bg-card pl-2 pr-2.5 text-sm font-semibold transition-all", category === k ? "border-foreground/40 shadow-[0_0_0_3px_color-mix(in_srgb,var(--foreground)_7%,transparent)]" : "hover:border-foreground/20")}>
            {k ? <CatDot category={k as string} size={28} round /> : <span className="grid size-7 place-items-center rounded-full bg-muted"><Layers className="size-3.5" /></span>}{l}<span className="grid h-6 min-w-6 place-items-center rounded-full bg-muted px-1.5 text-xs tabular-nums">{n ?? "—"}</span>
          </button>
        ))}
      </div>
      <Panel flush title={<>账号 <Count n={items.length} /></>} desc={pools.data ? `调度状态观测于 ${fmt.ago(pools.data.observed_at_ms)}` : undefined}
        action={<div className="flex flex-wrap items-center gap-2"><FilterSearch param="q" placeholder="搜索账号 / 邮箱" /><ToggleGroup type="single" value={issue} onValueChange={(v) => v && setIssue(v)} variant="outline" size="sm">{[["all", "全部"], ["attention", "需关注"], ["reauth", "需重新授权"], ["limited", "受限"], ["disabled", "已停用"]].map(([v, l]) => <ToggleGroupItem key={v} value={v} className="px-3 text-xs">{l}</ToggleGroupItem>)}</ToggleGroup></div>}>
        {sel.length > 0 && (
          <div className="mx-6 mb-3 flex flex-wrap items-center gap-2 rounded-xl border bg-muted/60 px-3 py-2 text-sm" role="toolbar" aria-label="批量操作">
            <b>已选 {sel.length}</b><span className="flex-1" />
            <Button size="sm" variant="outline" onClick={() => poolAction("request_recovery", sel)}><RefreshCw />请求恢复</Button>
            <Button size="sm" variant="outline" onClick={() => poolAction("cool_down", sel)}><Snowflake />冷却 10 分钟</Button>
            <Button size="sm" variant="outline" onClick={() => setEnabled(sel, true)}>启用</Button>
            <Button size="sm" variant="outline" onClick={() => setEnabled(sel, false)}>停用</Button>
            <Button size="icon" variant="ghost" className="size-8" onClick={() => setSel([])} aria-label="清除选择"><X /></Button>
          </div>
        )}
        <Q q={inventory} what="账号库存">{() => (
          <Table>
            <TableHeader><TableRow>
              <TableHead className="w-10 pl-6"><Checkbox checked={sel.length > 0 && sel.length === items.length} onCheckedChange={(v) => setSel(v ? items.map((i) => i.id) : [])} aria-label="全选" /></TableHead>
              <TableHead>账号</TableHead><TableHead>授权 / 调度</TableHead><TableHead className="min-w-[320px]">配额窗口</TableHead><TableHead className="text-right">优先级 · 权重</TableHead><TableHead className="text-right">并发</TableHead><TableHead className="pr-6" />
            </TableRow></TableHeader>
            <TableBody>
              {items.length === 0 && <TableRow><TableCell colSpan={7} className="py-10 text-center text-sm text-muted-foreground">没有符合条件的账号</TableCell></TableRow>}
              {items.map((i) => {
                const p = poolBy.get(i.id); const qq = quotaOf(i.id);
                return (
                  <TableRow key={i.id} data-state={sel.includes(i.id) ? "selected" : undefined} className={cn(i.status === "disabled" && "opacity-60")}>
                    <TableCell className="pl-6"><Checkbox checked={sel.includes(i.id)} onCheckedChange={() => toggle(i.id)} aria-label={`选择 ${i.id}`} /></TableCell>
                    <TableCell><button className="flex min-w-0 items-center gap-2.5 text-left" onClick={() => setParam("account", i.id)}><CatDot category={i.category} size={32} /><div className="min-w-0"><Mono className="block truncate font-semibold hover:underline">{i.id}</Mono><span className="flex flex-wrap items-center gap-1.5 text-xs text-muted-foreground"><span className="truncate">{i.identity.email ?? i.identity.username ?? i.provider}</span>{i.plan && <Badge variant="secondary" className="h-4 px-1.5 text-[10px]">{i.plan}</Badge>}{i.native && <Badge variant="outline" className="h-4 px-1.5 text-[10px]">native</Badge>}</span></div></button></TableCell>
                    <TableCell><div className="grid gap-1"><Status map="account_status" k={i.status} />{p ? <Status map="runtime_status" k={p.runtime_status} /> : <span className="text-xs text-muted-foreground">不在调度池</span>}</div></TableCell>
                    <TableCell>{i.category === "api" ? <span className="text-xs text-muted-foreground">API Key 渠道不报告配额，靠 429 冷却与熔断保护</span> : <QuotaWindows obs={qq?.data?.obs} error={qq?.data?.error} loading={qq?.isPending} />}</TableCell>
                    <TableCell className="text-right tabular-nums">{p ? `P${p.priority} · ${p.weight}` : "—"}</TableCell>
                    <TableCell className="text-right tabular-nums">{p ? `${p.active_leases} / ${p.max_concurrency}` : "—"}</TableCell>
                    <TableCell className="pr-6 text-right"><Button size="icon" variant="ghost" className="size-8" aria-label={`${i.id} 详情`} onClick={() => setParam("account", i.id)}><MoreHorizontal /></Button></TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        )}</Q>
      </Panel>
      <Failures />
      {open && <AccountSheet item={(inventory.data?.items ?? []).find((i) => i.id === open)} pool={poolBy.get(open)} onClose={() => setParam("account", null)} onAction={poolAction} onEnable={setEnabled} />}
    </>
  );
}

function Failures() {
  const f = useGet<S<"ProviderAccountFailurePage">>(["pool-failures"], "/admin/operations/provider-account-pools/failures", { query: { limit: 20 } }, { graph: false });
  return (
    <Panel flush title="最近的账号失败" desc="按账号归因的失败 Attempt；只含错误分类与重试决策">
      <Q q={f} what="账号失败记录">{(d) => d.items.length ? (
        <Table><TableHeader><TableRow><TableHead className="pl-6">时间</TableHead><TableHead>账号</TableHead><TableHead>错误</TableHead><TableHead>范围</TableHead><TableHead>重试决策</TableHead><TableHead className="pr-6">请求</TableHead></TableRow></TableHeader>
          <TableBody>{d.items.map((x) => <TableRow key={x.attempt_id}><TableCell className="pl-6"><Mono className="text-muted-foreground">{fmt.date(x.ended_at_ms)}</Mono></TableCell><TableCell><Mono>{x.account_id}</Mono></TableCell><TableCell><Mono className="text-xs" >{x.error_code}</Mono></TableCell><TableCell className="text-xs">{x.error_scope}</TableCell><TableCell className="text-xs">{x.retry_decision}</TableCell><TableCell className="pr-6"><button className="font-mono text-xs underline decoration-border underline-offset-2 hover:decoration-foreground" onClick={() => go("requests", { range: "7d", id: x.request_id })}>{x.request_id}</button></TableCell></TableRow>)}</TableBody></Table>
      ) : <p className="px-6 py-6 text-center text-sm text-muted-foreground">没有记录到账号失败</p>}</Q>
    </Panel>
  );
}

function AccountSheet({ item, pool, onClose, onAction, onEnable }: { item?: Item; pool?: Pool; onClose: () => void; onAction: (a: "request_recovery" | "cool_down", ids: string[]) => void; onEnable: (ids: string[], v: boolean) => void }) {
  const quota = useQuota(item ?? ({ id: "", native: false, category: "api" } as Item));
  if (!item) return null;
  return (
    <Sheet open onOpenChange={(o) => !o && onClose()}>
      <SheetContent className="w-[min(540px,100vw)] overflow-y-auto sm:max-w-[540px]">
        <SheetHeader><SheetTitle className="flex items-center gap-2"><CatDot category={item.category} size={26} /><Mono>{item.id}</Mono></SheetTitle><SheetDescription>{item.provider} · {CATEGORY[item.category]}{item.native ? " · native" : ""}</SheetDescription></SheetHeader>
        <div className="grid gap-5 px-4 pb-6">
          <div className="flex flex-wrap gap-3"><Status map="account_status" k={item.status} />{pool && <><Status map="auth_status" k={pool.auth_status} /><Status map="runtime_status" k={pool.runtime_status} /></>}</div>
          <section><h4 className="mb-2 text-xs font-semibold text-muted-foreground">身份</h4><KV rows={[["email", item.identity.email ?? "—"], ["username", item.identity.username ?? "—"], ["套餐", item.plan ? <span>{item.plan} <span className="text-xs text-muted-foreground">（{item.plan_source}）</span></span> : "未观测"], ["授权方式", item.managed?.authentication ?? (item.native ? "native OAuth" : "—")], ["凭据", item.managed ? <span>revision {item.managed.credential.revision} · <Badge variant="outline">AEAD · 只写</Badge></span> : item.native_account ? `revision ${item.native_account.revision}` : "—"]]} /></section>
          <section><h4 className="mb-2 text-xs font-semibold text-muted-foreground">配额窗口</h4><QuotaWindows obs={quota.data?.obs} error={quota.data?.error} loading={quota.isPending} />{quota.data?.obs && <p className="mt-2 text-xs text-muted-foreground">来源 {quota.data.obs.source} · 观测于 {fmt.ago(quota.data.obs.observed_at_ms)}{quota.data.obs.partial ? " · 部分窗口" : ""}</p>}</section>
          {pool && <section><h4 className="mb-2 text-xs font-semibold text-muted-foreground">调度</h4><KV rows={[["端点", <Mono>{pool.channel_id}</Mono>], ["优先级 / 权重", `P${pool.priority} · ${pool.weight}`], ["并发", `${pool.active_leases} 进行中 / 上限 ${pool.max_concurrency}`], ["权益", pool.entitlement ? `${pool.entitlement.tier} · ${pool.entitlement.confidence}` : "—"], ["凭据到期", pool.expires_at_ms ? fmt.date(pool.expires_at_ms) : "—"], ["下次配额同步", pool.quota_sync_due_at_ms ? fmt.date(pool.quota_sync_due_at_ms) : "—"]]} /></section>}
          {item.managed && <section><h4 className="mb-2 text-xs font-semibold text-muted-foreground">连接</h4><ul className="grid gap-1.5">{item.managed.connections.map((c) => <li key={c.id} className="flex items-center gap-2 text-sm"><Mono>{c.id}</Mono><span className="text-xs text-muted-foreground">{c.api_format} · {c.host ?? "—"}</span><span className="ml-auto"><Status map="group_status" k={c.enabled ? "active" : "disabled"} /></span></li>)}</ul></section>}
          <div className="flex flex-wrap gap-2">
            {pool && pool.runtime_status !== "available" && <Button variant="outline" size="sm" onClick={() => onAction("request_recovery", [item.id])}><RefreshCw />请求恢复</Button>}
            {pool && <Button variant="outline" size="sm" onClick={() => onAction("cool_down", [item.id])}><Snowflake />冷却 10 分钟</Button>}
            {item.operations.includes("reauthorize") && <Button variant="outline" size="sm" onClick={() => go("onboarding", { reauth: item.id, category: item.category })}>重新授权</Button>}
            <Button variant="ghost" size="sm" onClick={() => onEnable([item.id], item.status === "disabled")}>{item.status === "disabled" ? "启用" : "停用"}</Button>
          </div>
        </div>
      </SheetContent>
    </Sheet>
  );
}
