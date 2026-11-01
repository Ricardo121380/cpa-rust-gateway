// Configuration versions: a lineage track (oldest → newest, drafts dashed), per-version diff against the
// active graph, validate, publish (atomic) and roll back to the previous active version.
import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Check, GitFork, Plus, Send, Undo2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { forkDraft, refreshConfigContext, selectWorking, send, useConfigVersions, useGet, validateVersion, type S } from "@/api/hooks";
import { ApiError, describeError } from "@/api/errors";
import { useConfigContext } from "@/api/session";
import { toast } from "@/app/toast";
import { setParam } from "@/app/router";
import { Empty, KV, Mono, PageHeader, Panel, Q, Since, Status } from "@/components/kit";
import { useParam } from "@/components/table";
import { fmt, resourceKey } from "@/lib/format";
import { cn } from "@/lib/utils";
import { PublishDialog } from "./PublishDialog";

type V = S<"ConfigVersion">;

export function Versions() {
  const qc = useQueryClient();
  const versions = useConfigVersions();
  const ctx = useConfigContext();
  const sel = useParam("v") || ctx.working || "";
  const [publishing, setPublishing] = useState<V | null>(null);
  const [rolling, setRolling] = useState(false);
  const [validation, setValidation] = useState<{ id: string; r: S<"Validation"> } | null>(null);
  const list = versions.data ?? [];
  const v = list.find((x) => x.id === sel) ?? list[0];
  const wantsPublish = useParam("publish");
  useEffect(() => { if (wantsPublish && ctx.workingStatus === "draft") { const w = list.find((x) => x.id === ctx.working); if (w) setPublishing(w); setParam("publish", null); } }, [wantsPublish, list.length]);

  async function validate(id: string) {
    try { setValidation({ id, r: await validateVersion(id) }); }
    catch (e) { const d = describeError(e); toast({ title: d.title, description: d.detail, status: "error" }); }
  }
  async function fork(from: string) {
    try { const n = await forkDraft(from, `基于 ${from}`); toast({ title: `已创建草稿 ${n.id}`, status: "success" }); setParam("v", n.id); qc.invalidateQueries(); }
    catch (e) { const d = describeError(e); toast({ title: d.title, description: d.detail, status: "error" }); }
  }
  const ordered = [...list].sort((a, b) => a.created_at_ms - b.created_at_ms);

  return (
    <>
      <PageHeader title="配置版本" desc={<>草稿 → 验证 → 原子发布 → 回滚。写入带 <code>X-Config-Version</code> 和 <code>If-Match</code>，修订号不一致时返回 409，不会部分写入。</>} actions={<><Button variant="outline" onClick={() => setRolling(true)} disabled={!list.some((x) => x.status === "active" && x.parent_id)}><Undo2 />回滚到上一版本</Button>{ctx.active && <Button onClick={() => fork(ctx.active!)}><GitFork />从生效版本创建草稿</Button>}</>} />
      <Q q={versions} what="配置版本">{() => list.length === 0 ? <Panel><FirstVersion /></Panel> : (<>
        <Panel title="版本轨道" desc="从旧到新；实心为生效版本，虚线为草稿">
          <div className="flex overflow-x-auto pb-1" role="list">
            {ordered.map((x) => (
              <button key={x.id} role="listitem" onClick={() => setParam("v", x.id)} className={cn("relative min-w-[170px] flex-1 pt-7 text-left", "before:absolute before:left-0 before:right-0 before:top-[13px] before:h-0.5 before:bg-border first:before:left-2 last:before:right-auto last:before:w-2")} aria-current={x.id === v?.id}>
                <i className={cn("absolute left-0.5 top-[7px] size-3.5 rounded-full border-2 bg-card transition-transform", x.id === v?.id && "scale-125")} style={x.status === "active" ? { background: "var(--ok)", borderColor: "var(--ok)", boxShadow: "0 0 0 5px color-mix(in srgb, var(--ok) 20%, transparent)" } : x.status === "draft" ? { borderColor: "var(--info)", borderStyle: "dashed" } : { borderColor: "var(--muted-foreground)" }} />
                <div className="flex items-center gap-2"><Mono className={cn("font-semibold", x.id === v?.id && "underline underline-offset-4")}>{x.id}</Mono>{x.status !== "archived" && <Status map="cv_status" k={x.status} />}</div>
                <p className="mr-3 mt-1 line-clamp-2 text-xs text-muted-foreground">{x.description || "无描述"}</p>
                <span className="text-[11px] text-muted-foreground"><Mono>{x.revision}</Mono> · {fmt.ago(x.created_at_ms)}</span>
              </button>
            ))}
          </div>
        </Panel>
        {v && (
          <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)]">
            <Panel title={<span className="flex items-center gap-2"><Mono>{v.id}</Mono><Status map="cv_status" k={v.status} /></span>} desc={v.description || "无描述"}
              action={<div className="flex flex-wrap gap-1.5">
                {v.id !== ctx.working && v.status !== "archived" && <Button size="sm" variant="outline" onClick={() => { selectWorking(v); qc.invalidateQueries(); }}>切换到此版本</Button>}
                {v.status === "draft" && <><Button size="sm" variant="outline" onClick={() => validate(v.id)}><Check />验证</Button><Button size="sm" onClick={() => { selectWorking(v); setPublishing(v); }}><Send />发布</Button></>}
                {v.status !== "draft" && <Button size="sm" variant="outline" onClick={() => fork(v.id)}><GitFork />基于此创建草稿</Button>}
              </div>}>
              <KV rows={[["修订号（ETag）", <Mono>{v.revision}</Mono>], ["父版本", v.parent_id ? <button className="font-mono underline decoration-border underline-offset-2" onClick={() => setParam("v", v.parent_id!)}>{v.parent_id}</button> : "—"], ["创建于", <span>{fmt.date(v.created_at_ms)} <Since t={v.created_at_ms} /></span>]]} />
              {validation?.id === v.id && <div className="mt-4 flex items-start gap-3 rounded-xl p-3" style={{ background: `color-mix(in srgb, var(--${validation.r.valid ? "ok" : "bad"}) 10%, transparent)` }}>{validation.r.valid ? <span className="t-success-check" data-state="in" style={{ color: "var(--ok)" }}><svg viewBox="0 0 48 48" width="24" height="24" fill="none"><path d="M12 25l8 8 16-17" stroke="currentColor" strokeWidth="4" strokeLinecap="round" strokeLinejoin="round" /></svg></span> : null}<div><b>{validation.r.valid ? "验证通过" : "没有通过验证"}</b><ul className="text-xs text-muted-foreground">{(validation.r.error_codes ?? []).map((c) => <li key={c}>{describeError(new ApiError(409, c, c)).title} <Mono>{c}</Mono></li>)}</ul></div></div>}
            </Panel>
            <Diff v={v} base={ctx.active} />
          </div>
        )}
      </>)}</Q>
      {publishing && <PublishDialog version={publishing} onClose={() => setPublishing(null)} />}
      {rolling && <RollbackDialog onClose={() => setRolling(false)} active={list.find((x) => x.status === "active")} />}
    </>
  );
}

function Diff({ v, base }: { v: V; base: string | null }) {
  const vs = v.id === base ? v.parent_id : base;
  const diff = useGet<S<"ConfigurationDiffPage">>(["diff", v.id, vs, v.revision], "/admin/config-versions/{config_version_id}/diff", { path: { config_version_id: v.id }, query: { base_id: vs, limit: 200 } }, { enabled: !!vs, graph: false });
  if (!vs) return <Panel title="变更"><p className="text-sm text-muted-foreground">没有可比较的基线版本。</p></Panel>;
  return (
    <Panel flush title="变更" desc={<>相对 <Mono>{vs}</Mono> 的资源级差异（只比较结构，不含密钥内容）</>}>
      <Q q={diff} what="版本差异">{(d) => d.items.length ? (
        <ul className="max-h-[420px] overflow-y-auto">{d.items.map((c) => (
          <li key={c.resource_kind + c.resource_key} className="flex flex-wrap items-center gap-3 border-b px-6 py-2.5 text-sm last:border-0"><Status map="change" k={c.change} className="w-12" /><Mono className="text-xs text-muted-foreground">{c.resource_kind}</Mono><Mono className="flex-1 font-semibold">{resourceKey(c.resource_key)}</Mono>{c.changed_fields.length > 0 && <span className="text-xs text-muted-foreground">{c.changed_fields.join(", ")}</span>}</li>
        ))}</ul>
      ) : <p className="px-6 py-8 text-center text-sm text-muted-foreground">与基线没有差异</p>}</Q>
    </Panel>
  );
}

/** A fresh gateway has no configuration graph at all: the first version starts empty, as a draft. */
export function FirstVersion() {
  const qc = useQueryClient();
  const [busy, setBusy] = useState(false);
  async function create() {
    setBusy(true);
    try {
      const id = `cv-${new Date().toISOString().replace(/[-:TZ.]/g, "").slice(0, 14)}`;
      const v = await send<V>("POST", "/admin/config-versions", { body: { id, parent_id: null, description: "初始配置" } });
      await refreshConfigContext(); selectWorking(v);
      toast({ title: `已创建草稿 ${v.id}`, description: "在草稿里添加上游、模型和访问组，验证后发布即可生效", status: "success", duration: 6000 });
      qc.invalidateQueries();
    } catch (e) { const d = describeError(e); toast({ title: d.title, description: d.detail, status: "error" }); } finally { setBusy(false); }
  }
  return <Empty icon={<GitFork className="size-5" />} title="网关还没有配置版本" body="所有上游、模型、路由和访问组都属于某个配置版本。先创建一个空草稿，填好内容并验证，再发布为生效版本。" action={<Button size="sm" disabled={busy} onClick={create}><Plus />{busy ? "正在创建…" : "创建第一个配置版本"}</Button>} />;
}

function RollbackDialog({ active, onClose }: { active?: V; onClose: () => void }) {
  const qc = useQueryClient();
  const [busy, setBusy] = useState(false);
  async function rollback() {
    setBusy(true);
    try { const p = await send<S<"Publication">>("POST", "/admin/config-versions/rollback", { headers: active ? { "If-Match": `"${active.revision.replace(/"/g, "")}"` } : undefined }); toast({ title: `已回滚，生效版本 ${p.active_config_version_id}`, description: p.replaced_config_version_id ? `替换了 ${p.replaced_config_version_id}` : undefined, status: "success" }); await refreshConfigContext(); qc.invalidateQueries(); onClose(); }
    catch (e) { const d = describeError(e); toast({ title: d.title, description: d.detail, status: "error" }); } finally { setBusy(false); }
  }
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-[440px]">
        <DialogHeader><DialogTitle>回滚到上一个生效版本</DialogTitle><DialogDescription>当前生效 <Mono>{active?.id}</Mono>，回滚到 <Mono>{active?.parent_id ?? "—"}</Mono>。回滚是一次新的发布，进行中的请求继续使用旧路由快照。</DialogDescription></DialogHeader>
        <DialogFooter><Button variant="outline" onClick={onClose}>取消</Button><Button variant="destructive" disabled={busy} onClick={rollback}><Undo2 />{busy ? "正在回滚…" : "回滚"}</Button></DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
