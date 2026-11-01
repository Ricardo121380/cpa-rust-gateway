// Audit and backup. Two audit streams: configuration lifecycle (GET /admin/audit-events) and resource-level
// changes (GET /admin/resource-audit-events, paged by before_id). Backups are encrypted artifacts; restore
// accepts only an empty target and is checked by a preflight first. The artifact is streamed to the gateway
// as bytes and never parsed or displayed in the browser.
import { useRef, useState } from "react";
import { useInfiniteQuery } from "@tanstack/react-query";
import { ChevronsDown, FileUp, ShieldCheck } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { api, unwrap } from "@/api/client";
import { get, send, useGet, type S } from "@/api/hooks";
import { describeError } from "@/api/errors";
import { toast } from "@/app/toast";
import { KV, Mono, PageHeader, Panel, Q, Tonal } from "@/components/kit";
import { fmt } from "@/lib/format";

export function Audit() {
  const [tab, setTab] = useState("lifecycle");
  const events = useGet<S<"AuditEvent">[]>(["audit"], "/admin/audit-events", undefined, { graph: false });
  const resource = useInfiniteQuery({
    queryKey: ["resource-audit"], initialPageParam: null as string | null,
    queryFn: ({ pageParam }) => get<S<"ResourceAuditPage">>("/admin/resource-audit-events", { query: { limit: 50, before_id: pageParam ?? undefined } }),
    getNextPageParam: (p) => p.next_before_id,
  });
  const rows = resource.data?.pages.flatMap((p) => p.items) ?? [];
  return (
    <>
      <PageHeader title="审计与备份" desc="管理操作的审计记录只含动作、操作者和资源标识，不含凭据内容。" />
      <div className="grid gap-4 xl:grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)]">
        <Panel title="审计" action={<Tabs value={tab} onValueChange={setTab}><TabsList><TabsTrigger value="lifecycle">配置生命周期</TabsTrigger><TabsTrigger value="resource">资源变更</TabsTrigger></TabsList></Tabs>}>
          {tab === "lifecycle" ? (
            <Q q={events} what="审计事件">{(list) => <Timeline items={list.map((a) => ({ key: String(a.id), action: a.action, t: a.occurred_at_ms, meta: <><Mono>#{a.id}</Mono> · {a.actor} · <Mono>{a.config_version_id}</Mono>{a.replaced_config_version_id && <> 替换 <Mono>{a.replaced_config_version_id}</Mono></>}</> }))} />}</Q>
          ) : (
            <>
              <Timeline items={rows.map((a) => ({ key: a.id, action: a.action, t: a.occurred_at_ms, meta: <>{a.actor} · {a.resource_kind} <Mono>{a.resource_id}</Mono> · <Mono>{a.config_version_id}</Mono></> }))} />
              {resource.hasNextPage && <div className="mt-3 flex justify-center"><Button size="sm" variant="outline" onClick={() => resource.fetchNextPage()} disabled={resource.isFetchingNextPage}><ChevronsDown />加载更早的记录</Button></div>}
            </>
          )}
        </Panel>
        <div className="grid content-start gap-4"><Backup /><Restore /></div>
      </div>
    </>
  );
}

function Timeline({ items }: { items: { key: string; action: string; t: number; meta: React.ReactNode }[] }) {
  if (!items.length) return <p className="py-6 text-center text-sm text-muted-foreground">没有记录</p>;
  return (
    <ol className="relative">{items.map((a, i) => (
      <li key={a.key} className="relative grid grid-cols-[14px_1fr_auto] gap-3 py-2.5">
        {i < items.length - 1 && <span className="absolute bottom-[-10px] left-[6px] top-6 w-0.5 bg-border" />}
        <i className="mt-1 size-3.5 rounded-full border-[3px] border-card" style={{ background: a.action.includes("publish") ? "var(--ok)" : a.action.includes("rollback") || a.action.includes("revoke") ? "var(--warn)" : "var(--muted-foreground)", boxShadow: "0 0 0 1px var(--border)" }} />
        <div className="min-w-0"><Mono className="font-semibold">{a.action}</Mono><p className="truncate text-xs text-muted-foreground">{a.meta}</p></div>
        <time className="text-xs text-muted-foreground" dateTime={new Date(a.t).toISOString()} title={new Date(a.t).toLocaleString()}>{fmt.date(a.t)}</time>
      </li>
    ))}</ol>
  );
}

function Backup() {
  const [r, setR] = useState<S<"BackupPreflight"> | null>(null);
  const [busy, setBusy] = useState(false);
  async function preflight() {
    setBusy(true);
    try { setR(await send<S<"BackupPreflight">>("POST", "/admin/backups/preflight")); } catch (e) { const d = describeError(e); toast({ title: d.title, description: d.detail, status: "error" }); } finally { setBusy(false); }
  }
  return (
    <Panel title="备份" desc="备份包含配置图和加密凭据；生成备份文件在网关 CLI 上完成" action={<Button size="sm" variant="outline" disabled={busy} onClick={preflight}><ShieldCheck />预检</Button>}>
      {r ? <KV rows={[["schema_version", r.schema_version], ["需要主密钥", r.secret_key_required ? <Tonal tone="warn">是：恢复时需要同一把外部主密钥</Tonal> : "否"]]} /> : <p className="text-sm text-muted-foreground">预检会确认当前存储可以导出，并说明恢复时需要什么。</p>}
    </Panel>
  );
}

function Restore() {
  const input = useRef<HTMLInputElement>(null);
  const [file, setFile] = useState<File | null>(null);
  const [r, setR] = useState<S<"RestorePreflight"> | null>(null);
  const [busy, setBusy] = useState(false);
  async function preflight(f: File) {
    setFile(f); setR(null); setBusy(true);
    try {
      const body = await f.arrayBuffer();
      setR(await unwrap<S<"RestorePreflight">>(api.POST("/admin/restores/preflight", { body: body as never, bodySerializer: (b: unknown) => b as BodyInit, headers: { "Content-Type": "application/octet-stream" } } as never) as never));
    } catch (e) { const d = describeError(e); toast({ title: d.title, description: d.detail, status: "error" }); } finally { setBusy(false); }
  }
  return (
    <Panel title="恢复" desc="只接受空的目标数据库；先预检再恢复">
      <input ref={input} type="file" accept=".cparbak,application/octet-stream" className="sr-only" tabIndex={-1} aria-label="选择备份文件" onChange={(e) => e.target.files?.[0] && preflight(e.target.files[0])} />
      <button onClick={() => input.current?.click()} className="grid w-full place-items-center gap-1.5 rounded-2xl border-[1.5px] border-dashed p-6 text-center transition-colors hover:bg-muted" disabled={busy}>
        <FileUp className="size-5" /><b className="text-sm">{file ? file.name : "选择备份文件"}</b><span className="text-xs text-muted-foreground">文件只作为字节发送给网关做预检，浏览器不读取内容</span>
      </button>
      {busy && <span className="t-shimmer mt-3 block text-xs" data-text="正在预检…">正在预检…</span>}
      {r && <div className="mt-4"><KV rows={[["schema_version", r.schema_version], ["兼容", r.compatible ? <Tonal tone="ok">兼容</Tonal> : <Tonal tone="bad">不兼容</Tonal>], ["需要 quick_check", r.quick_check_required ? "是" : "否"]]} /><p className="mt-3 text-xs text-muted-foreground">执行恢复需要网关处于空目标状态，并由 CLI 提供主密钥；控制台不发起实际恢复。</p></div>}
    </Panel>
  );
}
