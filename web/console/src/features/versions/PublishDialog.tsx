import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, Send } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { get, refreshConfigContext, send, validateVersion, type S } from "@/api/hooks";
import { ApiError, describeError } from "@/api/errors";
import { useConfigContext } from "@/api/session";
import { toast } from "@/app/toast";
import { KV, Mono, Status, Tonal } from "@/components/kit";
import { resourceKey } from "@/lib/format";

/** Validate first, show the diff against the active graph, then publish atomically. */
export function PublishDialog({ version, onClose }: { version: S<"ConfigVersion">; onClose: () => void }) {
  const ctx = useConfigContext();
  const qc = useQueryClient();
  const [busy, setBusy] = useState(false);
  // Publish carries If-Match of this version: re-read it first so a write made elsewhere is not a surprise 409.
  useEffect(() => { refreshConfigContext().catch(() => undefined); }, [version.id]);
  const validation = useQuery({ queryKey: ["validate", version.id, ctx.revision], queryFn: () => validateVersion(version.id) });
  const diff = useQuery({ queryKey: ["diff", version.id, ctx.active, ctx.revision], enabled: !!ctx.active, queryFn: () => get<S<"ConfigurationDiffPage">>("/admin/config-versions/{config_version_id}/diff", { path: { config_version_id: version.id }, query: { base_id: ctx.active, limit: 100 } }) });
  const valid = validation.data?.valid;

  async function publish() {
    setBusy(true);
    try {
      const p = await send<S<"Publication">>("POST", "/admin/config-versions/{config_version_id}/publish", { path: { config_version_id: version.id } });
      toast({ title: `已发布 ${p.active_config_version_id}`, description: p.replaced_config_version_id ? `替换了 ${p.replaced_config_version_id}，进行中的请求继续使用旧路由快照` : undefined, status: "success" });
      await refreshConfigContext();
      qc.invalidateQueries();
      onClose();
    } catch (e) { const d = describeError(e); toast({ title: d.title, description: d.detail, status: "error", duration: 7000 }); } finally { setBusy(false); }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-[560px]">
        <DialogHeader><DialogTitle>发布 {version.id}</DialogTitle><DialogDescription>{ctx.active ? <>原子替换当前生效版本 <Mono>{ctx.active}</Mono>，路由快照会重新编译。</> : "这是网关的第一个生效版本，发布后路由快照按它编译。"}</DialogDescription></DialogHeader>
        <KV rows={[["描述", version.description || "—"], ["修订号", <Mono>{ctx.revision}</Mono>], ["验证", validation.isPending ? <span className="t-shimmer text-xs" data-text="正在验证…">正在验证…</span> : validation.isError ? <Tonal tone="bad">{describeError(validation.error).title}</Tonal> : valid ? <Tonal tone="ok">通过</Tonal> : <Tonal tone="bad">未通过</Tonal>]]} />
        {validation.data && !valid && <div className="flex gap-2.5 rounded-xl p-3 text-sm" style={{ background: "color-mix(in srgb, var(--bad) 10%, transparent)" }}><AlertTriangle className="mt-0.5 size-4 shrink-0" style={{ color: "var(--bad)" }} /><div><b>验证未通过，不能发布</b><ul className="mt-1 text-xs text-muted-foreground">{validation.data.error_codes?.map((c) => <li key={c}>{describeError(new ApiError(409, c, c)).title} <Mono>{c}</Mono></li>)}</ul></div></div>}
        <div>
          <h4 className="mb-2 text-xs font-semibold text-muted-foreground">{ctx.active ? `相对 ${ctx.active} 的变更` : "变更"}</h4>
          <div className="max-h-56 overflow-y-auto rounded-xl border">
            {diff.data?.items.length ? diff.data.items.map((c) => (
              <div key={c.resource_kind + c.resource_key} className="flex items-center gap-3 border-b px-3 py-2 text-xs last:border-0"><Status map="change" k={c.change} /><Mono className="text-muted-foreground">{c.resource_kind}</Mono><Mono className="flex-1 truncate font-semibold">{resourceKey(c.resource_key)}</Mono><span className="truncate text-muted-foreground">{c.changed_fields.join(", ")}</span></div>
            )) : <p className="p-4 text-center text-xs text-muted-foreground">{!ctx.active ? "没有生效版本可比较：草稿里的全部资源都会随发布生效" : diff.isPending ? "正在比较…" : "没有变更"}</p>}
          </div>
        </div>
        <DialogFooter><Button variant="outline" onClick={onClose}>取消</Button><Button disabled={!valid || busy} onClick={publish}><Send />{busy ? "正在发布…" : "发布"}</Button></DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
