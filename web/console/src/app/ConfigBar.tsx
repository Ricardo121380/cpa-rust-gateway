// The configuration context every read and write targets: active graph → working draft.
// Reads follow the working version; writes go to a draft (forked on first edit). Validate and publish live here.
import { useState } from "react";
import { Check, ChevronDown, GitFork, Plus, Send } from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { forkDraft, selectWorking, useConfigVersions, validateVersion } from "@/api/hooks";
import { useConfigContext } from "@/api/session";
import { ApiError, describeError } from "@/api/errors";
import { toast } from "./toast";
import { go } from "./router";
import { PublishDialog } from "@/features/versions/PublishDialog";
import { cn } from "@/lib/utils";

const describeCodes = (codes?: string[]) => codes?.map((c) => describeError(new ApiError(409, c, c)).title).join("；");

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
      const r = await validateVersion(ctx.working);
      toast(r.valid ? { title: `${ctx.working} 验证通过`, description: "没有发现结构性错误", status: "success" } : { title: `${ctx.working} 没有通过验证`, description: describeCodes(r.error_codes), status: "error", duration: 8000 });
    } catch (e) { const d = describeError(e); toast({ title: d.title, description: d.detail, status: "error" }); }
  }
  async function fork() {
    if (!ctx.active) return;
    try { const v = await forkDraft(ctx.active, "控制台编辑"); toast({ title: `已创建草稿 ${v.id}`, status: "success" }); qc.invalidateQueries(); }
    catch (e) { const d = describeError(e); toast({ title: d.title, description: d.detail, status: "error" }); }
  }

  if (ctx.loaded && versions.isSuccess && !list.length) return (
    <div className="flex min-w-0 items-center gap-1 rounded-xl border bg-card px-1 py-1 text-xs shadow-xs">
      <span className="hidden items-center gap-1.5 px-2 sm:inline-flex"><i className="size-2 rounded-full border border-dashed" style={{ borderColor: "var(--muted-foreground)" }} /><span className="text-muted-foreground">没有配置版本</span></span>
      <Button size="sm" variant="ghost" className="h-7" onClick={() => go("versions")} aria-label="创建第一个配置版本"><Plus /><span className="hidden md:inline">创建配置版本</span></Button>
    </div>
  );

  return (
    <div className="flex min-w-0 items-center gap-1 rounded-xl border bg-card px-1 py-1 text-xs shadow-xs">
      <span className="hidden items-center gap-1.5 px-2 sm:inline-flex" title="当前生效的配置版本"><i className="size-2 rounded-full" style={{ background: "var(--ok)" }} /><span className="text-muted-foreground">生效</span><b className="font-mono">{ctx.active ?? "—"}</b></span>
      <span className="hidden text-muted-foreground sm:inline" aria-hidden>→</span>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button className="flex min-w-0 items-center gap-1.5 rounded-lg px-2 py-1 hover:bg-muted" aria-label="切换正在查看的配置版本">
            <i className={cn("size-2 shrink-0 rounded-full", isDraft ? "border border-dashed" : "")} style={isDraft ? { borderColor: "var(--info)" } : { background: "var(--ok)" }} />
            <span className="hidden text-muted-foreground sm:inline">{isDraft ? "草稿" : "查看"}</span><b className="max-w-[9rem] truncate font-mono sm:max-w-none">{ctx.working ?? "—"}</b>
            {!compact && ctx.revision && <span className="hidden font-mono text-muted-foreground md:inline" title="If-Match 修订号">{ctx.revision}</span>}
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
          <Button size="sm" variant="ghost" className="h-7" onClick={validate} aria-label="验证草稿"><Check /><span className="hidden sm:inline">验证</span></Button>
          <Button size="sm" className="h-7" onClick={() => setPublishing(true)} aria-label="发布草稿"><Send /><span className="hidden sm:inline">发布</span></Button>
        </>
      ) : (
        <Button size="sm" variant="ghost" className="h-7" onClick={fork} title="修改配置前需要先创建草稿" aria-label="创建草稿"><GitFork /><span className="hidden md:inline">创建草稿</span></Button>
      )}
      {publishing && working && <PublishDialog version={working} onClose={() => setPublishing(false)} />}
    </div>
  );
}
