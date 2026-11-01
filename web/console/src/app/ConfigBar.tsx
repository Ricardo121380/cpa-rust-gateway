// The configuration context every read and write targets: active graph → working draft.
// Reads follow the working version; writes go to a draft (forked on first edit). Validate and publish live here.
import { useState } from "react";
import { Check, ChevronDown, GitFork, Send } from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { forkDraft, selectWorking, send, useConfigVersions, type S } from "@/api/hooks";
import { useConfigContext } from "@/api/session";
import { describeError } from "@/api/errors";
import { toast } from "./toast";
import { PublishDialog } from "@/features/versions/PublishDialog";
import { cn } from "@/lib/utils";

export function ConfigBar({ compact }: { compact?: boolean }) {
  const versions = useConfigVersions();
  const ctx = useConfigContext();
  const qc = useQueryClient();
  const [publishing, setPublishing] = useState(false);
  const list = versions.data ?? [];
  const working = list.find((v) => v.id === ctx.working);
  const isDraft = ctx.workingStatus === "draft";

  async function validate() {
    if (!ctx.working) return;
    try {
      const r = await send<S<"Validation">>("POST", "/admin/config-versions/{config_version_id}/validate", { path: { config_version_id: ctx.working } });
      toast(r.valid ? { title: `${ctx.working} 验证通过`, description: "没有发现结构性错误", status: "success" } : { title: `${ctx.working} 有 ${r.error_codes?.length ?? 0} 个问题`, description: r.error_codes?.join(", "), status: "error", duration: 8000 });
    } catch (e) { const d = describeError(e); toast({ title: d.title, description: d.detail, status: "error" }); }
  }
  async function fork() {
    if (!ctx.active) return;
    try { const v = await forkDraft(ctx.active, "控制台编辑"); toast({ title: `已创建草稿 ${v.id}`, status: "success" }); qc.invalidateQueries(); }
    catch (e) { const d = describeError(e); toast({ title: d.title, description: d.detail, status: "error" }); }
  }

  return (
    <div className="flex min-w-0 items-center gap-1 rounded-xl border bg-card px-1 py-1 text-xs shadow-xs">
      <span className="hidden items-center gap-1.5 px-2 sm:inline-flex" title="当前生效的配置版本"><i className="size-2 rounded-full" style={{ background: "var(--ok)" }} /><span className="text-muted-foreground">生效</span><b className="font-mono">{ctx.active ?? "—"}</b></span>
      <span className="hidden text-muted-foreground sm:inline" aria-hidden>→</span>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button className="flex min-w-0 items-center gap-1.5 rounded-lg px-2 py-1 hover:bg-muted" aria-label="切换正在查看的配置版本">
            <i className={cn("size-2 shrink-0 rounded-full", isDraft ? "border border-dashed" : "")} style={isDraft ? { borderColor: "var(--info)" } : { background: "var(--ok)" }} />
            <span className="text-muted-foreground">{isDraft ? "草稿" : "查看"}</span><b className="truncate font-mono">{ctx.working ?? "—"}</b>
            {!compact && ctx.revision && <span className="font-mono text-muted-foreground" title="If-Match 修订号">{ctx.revision}</span>}
            <ChevronDown className="size-3.5 text-muted-foreground" />
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="w-80">
          <DropdownMenuLabel>查看或编辑的版本</DropdownMenuLabel>
          {list.filter((v) => v.status !== "archived").map((v) => (
            <DropdownMenuItem key={v.id} onSelect={() => { selectWorking(v); qc.invalidateQueries(); }}>
              <div className="min-w-0 flex-1"><div className="flex items-center gap-2"><b className="font-mono">{v.id}</b><span className="text-xs text-muted-foreground">{v.status === "draft" ? "草稿" : "生效中"}</span></div><div className="truncate text-xs text-muted-foreground">{v.description || "无描述"}</div></div>
              {v.id === ctx.working && <Check className="size-4" />}
            </DropdownMenuItem>
          ))}
          <DropdownMenuSeparator />
          <DropdownMenuItem onSelect={fork}><GitFork />从生效版本创建新草稿</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      {isDraft ? (
        <>
          <Button size="sm" variant="ghost" className="h-7" onClick={validate}><Check />验证</Button>
          <Button size="sm" className="h-7" onClick={() => setPublishing(true)}><Send />发布</Button>
        </>
      ) : (
        <Button size="sm" variant="ghost" className="h-7" onClick={fork} title="修改配置前需要先创建草稿"><GitFork /><span className="hidden md:inline">创建草稿</span></Button>
      )}
      {publishing && working && <PublishDialog version={working} onClose={() => setPublishing(false)} />}
    </div>
  );
}
