// Access control. A client key belongs to one access group; the group's route grants decide which public
// models it sees. Keys are stored as HMAC: the plaintext is returned once, at issue time, and never again.
import { useMemo, useState } from "react";
import { Check, Copy, KeyRound, Plus, ShieldAlert } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { useQueries } from "@tanstack/react-query";
import { ensureDraft, get, send, useDraftMutation, useGet, useVersionKey, type S } from "@/api/hooks";
import { describeError } from "@/api/errors";
import { toast } from "@/app/toast";
import { Count, Mono, PageHeader, Panel, Q, Since, Status } from "@/components/kit";
import { fmt } from "@/lib/format";
import { useRouteLabels } from "@/features/routing/Routing";
import { cn } from "@/lib/utils";

type Key = S<"ClientKey">;
const keyState = (k: Key) => (k.status === "active" && k.expires_at_ms && k.expires_at_ms < Date.now() ? "expired" : k.status);

export function Access() {
  const groups = useGet<S<"AccessGroup">[]>(["access-groups"], "/admin/access-groups");
  const keys = useGet<Key[]>(["client-keys"], "/admin/client-keys");
  const routes = useRouteLabels();
  const [issuing, setIssuing] = useState(false);
  const [revoking, setRevoking] = useState<Key | null>(null);
  const setKeyStatus = useDraftMutation(async (k: Key) => { await ensureDraft(); await send("PATCH", "/admin/client-keys/{client_key_id}", { path: { client_key_id: k.id }, body: { id: k.id, access_group_id: k.access_group_id, status: k.status === "active" ? "disabled" : "active", expires_at_ms: k.expires_at_ms ?? null } }); }, { invalidate: [["client-keys"]], success: (k) => `${k.id} 已${k.status === "active" ? "停用" : "启用"}（草稿）` });

  return (
    <>
      <PageHeader title="访问控制" desc="Client Key 属于一个访问组，访问组授权哪些路由，Key 就只能看到哪些模型。修改写入当前草稿。" actions={<Button onClick={() => setIssuing(true)}><KeyRound />签发 Client Key</Button>} />
      <Q q={groups} what="访问组">{(gs) => <GrantMatrix groups={gs} routes={routes} />}</Q>
      <Panel flush title={<>Client Keys <Count n={keys.data?.length} /></>} desc="服务端只保存 HMAC 摘要与前缀">
        <Q q={keys} what="Client Key">{(ks) => (
          <Table>
            <TableHeader><TableRow><TableHead className="pl-6">Key</TableHead><TableHead>访问组</TableHead><TableHead>状态</TableHead><TableHead>过期</TableHead><TableHead>最近请求</TableHead><TableHead className="text-right">启用</TableHead><TableHead className="pr-6" /></TableRow></TableHeader>
            <TableBody>{ks.map((k) => {
              const st = keyState(k);
              return (
                <TableRow key={k.id} className={cn(st === "revoked" && "opacity-55")}>
                  <TableCell className="pl-6"><Mono className="font-semibold">{k.id}</Mono><div className="text-xs text-muted-foreground"><Mono>{k.prefix}…</Mono></div></TableCell>
                  <TableCell>{groups.data?.find((g) => g.id === k.access_group_id)?.name ?? <Mono>{k.access_group_id}</Mono>}</TableCell>
                  <TableCell><Status map="key_status" k={st} /></TableCell>
                  <TableCell className="text-xs">{k.expires_at_ms ? <span style={k.expires_at_ms < Date.now() ? { color: "var(--bad)" } : undefined}>{fmt.date(k.expires_at_ms)}</span> : <span className="text-muted-foreground">永不</span>}</TableCell>
                  <TableCell><Since t={k.last_request_at_ms} /></TableCell>
                  <TableCell className="text-right"><Switch checked={k.status === "active"} disabled={st === "revoked" || setKeyStatus.isPending} onCheckedChange={() => setKeyStatus.mutate(k)} aria-label={`${k.id} 启用`} /></TableCell>
                  <TableCell className="pr-6 text-right">{st !== "revoked" && <Button size="sm" variant="ghost" className="text-destructive hover:text-destructive" onClick={() => setRevoking(k)}>吊销</Button>}</TableCell>
                </TableRow>
              );
            })}</TableBody>
          </Table>
        )}</Q>
      </Panel>
      {issuing && <IssueDialog groups={groups.data ?? []} onClose={() => setIssuing(false)} />}
      {revoking && <RevokeDialog k={revoking} onClose={() => setRevoking(null)} />}
    </>
  );
}

function GrantMatrix({ groups, routes }: { groups: S<"AccessGroup">[]; routes: ReturnType<typeof useRouteLabels> }) {
  const v = useVersionKey();
  const perGroup = useQueries({ queries: groups.map((g) => ({ queryKey: ["grants", g.id, v], queryFn: () => get<S<"AccessGroupRoute">[]>("/admin/access-groups/{access_group_id}/routes", { path: { access_group_id: g.id } }) })) });
  const toggle = useDraftMutation(async ({ g, r, on }: { g: string; r: string; on: boolean }) => { await ensureDraft(); await send("POST", "/admin/access-groups/{access_group_id}/routes", { path: { access_group_id: g }, body: { route_id: r, enabled: !on } }); }, { invalidate: [["grants"]], success: ({ g, r, on }) => `${on ? "撤销" : "授予"} ${g} → ${r}（草稿）` });
  return (
    <Panel flush title="路由授权" desc="行是访问组，列是公开模型；点击格子授予或撤销">
      <div className="overflow-x-auto">
        <Table>
          <TableHeader><TableRow><TableHead className="pl-6">访问组</TableHead>{routes.map(({ route, model }) => <TableHead key={route.id} className="text-center"><Mono className="text-xs">{model?.model_name ?? route.id}</Mono></TableHead>)}</TableRow></TableHeader>
          <TableBody>{groups.map((g, gi) => {
            const gr = perGroup[gi].data ?? [];
            return (
              <TableRow key={g.id}>
                <TableCell className="pl-6"><b>{g.name}</b><div className="mt-0.5 flex flex-wrap items-center gap-2 text-xs"><Status map="group_status" k={g.status} /><span className="text-muted-foreground">{Object.entries(g.limits).map(([k, v]) => `${k} ${fmt.n(v)}`).join(" · ") || "无限额"}</span></div></TableCell>
                {routes.map(({ route }) => {
                  const on = gr.some((x) => x.route_id === route.id && x.enabled);
                  return (
                    <TableCell key={route.id} className="text-center">
                      <button aria-pressed={on} aria-label={`${g.name} → ${route.id}`} disabled={toggle.isPending || perGroup[gi].isPending} onClick={() => toggle.mutate({ g: g.id, r: route.id, on })} className={cn("inline-grid size-7 place-items-center rounded-lg transition-all hover:scale-110 disabled:opacity-50", on ? "text-white" : "border border-dashed border-input")} style={on ? { background: "var(--ok)" } : undefined}>{on && <Check className="size-3.5" />}</button>
                    </TableCell>
                  );
                })}
              </TableRow>
            );
          })}</TableBody>
        </Table>
      </div>
    </Panel>
  );
}

function IssueDialog({ groups, onClose }: { groups: S<"AccessGroup">[]; onClose: () => void }) {
  const [id, setId] = useState(`ck-${Date.now().toString(36).slice(-6)}`);
  const [group, setGroup] = useState(groups.find((g) => g.status === "active")?.id ?? "");
  const [days, setDays] = useState("90");
  const [issued, setIssued] = useState<S<"IssuedClientKey"> | null>(null);
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);
  const expires = useMemo(() => (days === "never" ? null : Date.now() + Number(days) * 864e5), [days]);
  async function issue() {
    setBusy(true);
    try { await ensureDraft(); setIssued(await send<S<"IssuedClientKey">>("POST", "/admin/client-keys", { body: { id, access_group_id: group, status: "active", expires_at_ms: expires } })); }
    catch (e) { const d = describeError(e); toast({ title: d.title, description: d.detail, status: "error" }); } finally { setBusy(false); }
  }
  return (
    <Dialog open onOpenChange={(o) => { if (!o && (!issued || copied)) onClose(); }}>
      <DialogContent className="sm:max-w-[480px]" onInteractOutside={(e) => issued && !copied && e.preventDefault()}>
        {!issued ? (<>
          <DialogHeader><DialogTitle>签发 Client Key</DialogTitle><DialogDescription>明文只在签发后显示一次。Key 写入草稿，发布后才能使用。</DialogDescription></DialogHeader>
          <div className="grid gap-3">
            <div className="grid gap-1.5"><Label htmlFor="kid">ID</Label><Input id="kid" value={id} onChange={(e) => setId(e.target.value)} /></div>
            <div className="grid gap-1.5"><Label>访问组</Label><Select value={group} onValueChange={setGroup}><SelectTrigger className="w-full"><SelectValue placeholder="选择访问组" /></SelectTrigger><SelectContent>{groups.map((g) => <SelectItem key={g.id} value={g.id} disabled={g.status !== "active"}>{g.name}{g.status !== "active" ? "（已停用）" : ""}</SelectItem>)}</SelectContent></Select></div>
            <div className="grid gap-1.5"><Label>有效期</Label><Select value={days} onValueChange={setDays}><SelectTrigger className="w-full"><SelectValue /></SelectTrigger><SelectContent>{[["30", "30 天"], ["90", "90 天"], ["365", "1 年"], ["never", "永不过期"]].map(([v, l]) => <SelectItem key={v} value={v}>{l}</SelectItem>)}</SelectContent></Select></div>
          </div>
          <DialogFooter><Button variant="outline" onClick={onClose}>取消</Button><Button disabled={busy || !id || !group} onClick={issue}><Plus />{busy ? "正在签发…" : "签发"}</Button></DialogFooter>
        </>) : (<>
          <DialogHeader><DialogTitle>Key 已签发</DialogTitle><DialogDescription>关闭后无法再次查看明文。请现在复制并保存到客户端。</DialogDescription></DialogHeader>
          <div className="flex gap-2.5 rounded-xl p-3 text-sm" style={{ background: "color-mix(in srgb, var(--warn) 12%, transparent)" }}><ShieldAlert className="mt-0.5 size-4 shrink-0" style={{ color: "var(--warn)" }} /><span>这是唯一一次显示。服务端只保存摘要，忘记后只能吊销并重新签发。</span></div>
          <div className="flex items-center gap-2 rounded-lg bg-muted p-2 pl-3"><Mono className="flex-1 select-all break-all text-[13px] font-semibold">{issued.key}</Mono><Button size="sm" variant={copied ? "secondary" : "default"} onClick={() => navigator.clipboard?.writeText(issued.key).then(() => setCopied(true))}>{copied ? <Check /> : <Copy />}{copied ? "已复制" : "复制"}</Button></div>
          <div className="flex flex-wrap gap-2 text-xs text-muted-foreground"><Badge variant="outline">{issued.id}</Badge><Badge variant="outline">{issued.prefix}…</Badge><Badge variant="outline">{issued.access_group_id}</Badge><Badge variant="outline">{issued.expires_at_ms ? `至 ${fmt.date(issued.expires_at_ms)}` : "永不过期"}</Badge></div>
          <DialogFooter><Button onClick={onClose} disabled={!copied} title={copied ? undefined : "先复制 Key"}>我已保存</Button></DialogFooter>
        </>)}
      </DialogContent>
    </Dialog>
  );
}

function RevokeDialog({ k, onClose }: { k: Key; onClose: () => void }) {
  const [typed, setTyped] = useState("");
  const revoke = useDraftMutation(async () => { await ensureDraft(); await send("DELETE", "/admin/client-keys/{client_key_id}", { path: { client_key_id: k.id } }); }, { invalidate: [["client-keys"]], success: `${k.id} 已吊销（草稿，发布后生效）` });
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-[440px]">
        <DialogHeader><DialogTitle>吊销 {k.id}</DialogTitle><DialogDescription>发布后，使用 <Mono>{k.prefix}…</Mono> 的客户端会立即收到 401。吊销不可撤销。</DialogDescription></DialogHeader>
        <div className="grid gap-1.5"><Label htmlFor="rv">输入 Key ID 以确认</Label><Input id="rv" value={typed} onChange={(e) => setTyped(e.target.value)} placeholder={k.id} autoComplete="off" /></div>
        <DialogFooter><Button variant="outline" onClick={onClose}>取消</Button><Button variant="destructive" disabled={typed !== k.id || revoke.isPending} onClick={() => revoke.mutate(undefined, { onSuccess: onClose })}>吊销</Button></DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
