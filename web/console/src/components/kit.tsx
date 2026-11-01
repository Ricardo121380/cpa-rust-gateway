// Shared presentational atoms. Everything reads design tokens; no colors are hard-coded here.
import type { ReactNode } from "react";
import type { UseQueryResult } from "@tanstack/react-query";
import { AlertTriangle, PlugZap, RefreshCw } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { describeError, isUnavailable } from "@/api/errors";
import { fmt } from "@/lib/format";
import { GLYPH, label, slotOf, CATEGORY, type Tone } from "@/lib/labels";
import { cn } from "@/lib/utils";

export const toneColor = (t: Tone) => (t === "off" ? "var(--muted-foreground)" : `var(--${t})`);

export function Status({ map, k, className }: { map: string; k: string | null | undefined; className?: string }) {
  const [text, tone] = label(map, k);
  return <span className={cn("inline-flex items-center gap-1.5 whitespace-nowrap text-xs font-medium", className)} style={{ color: toneColor(tone) }}><span className="text-[9px] leading-none" aria-hidden>{GLYPH[tone]}</span>{text}</span>;
}
export function Tonal({ tone, children, className }: { tone: Tone; children: ReactNode; className?: string }) {
  return <span className={cn("inline-flex items-center gap-1.5 whitespace-nowrap text-xs font-medium", className)} style={{ color: toneColor(tone) }}><span className="text-[9px]" aria-hidden>{GLYPH[tone]}</span>{children}</span>;
}
export function Conf({ c, prefix }: { c: string | null | undefined; prefix?: string }) {
  const [text, tone] = label("confidence", c);
  return <span title={`${prefix ?? "confidence"}: ${c}`} className="rounded border border-dashed px-1.5 py-0.5 text-[10px] font-semibold leading-none" style={{ color: toneColor(tone) }}>{text}</span>;
}
export function Mono({ children, className, title }: { children: ReactNode; className?: string; title?: string }) {
  return <span title={title} className={cn("font-mono text-[0.94em] tracking-tight", className)}>{children}</span>;
}
/** Category badge: two letters on the category's fixed categorical color. */
export function CatDot({ category, size = 22, round }: { category: string | null | undefined; size?: number; round?: boolean }) {
  const c = category ?? "api";
  const short = { codex: "CX", grok: "GK", claude: "CL", api: "AP", kiro: "KR", kimi: "KM" }[c] ?? c.slice(0, 2).toUpperCase();
  return <span title={CATEGORY[c] ?? c} className={cn("inline-grid shrink-0 place-items-center font-semibold text-white", round ? "rounded-full" : "rounded-[7px]")} style={{ width: size, height: size, background: `var(--c${slotOf(c)})`, fontSize: Math.round(size * 0.38) }}>{short}</span>;
}
export function KV({ rows }: { rows: [ReactNode, ReactNode][] }) {
  return <dl className="grid grid-cols-[minmax(120px,max-content)_1fr] gap-x-5 gap-y-2 text-[13px]">{rows.map(([k, v], i) => <div key={i} className="contents"><dt className="text-muted-foreground">{k}</dt><dd className="min-w-0 break-words">{v ?? "—"}</dd></div>)}</dl>;
}
export function PageHeader({ title, desc, actions }: { title: ReactNode; desc?: ReactNode; actions?: ReactNode }) {
  return (
    <header className="mb-5 flex flex-wrap items-end justify-between gap-4">
      <div className="min-w-0"><h1 className="text-[24px] font-bold leading-tight">{title}</h1>{desc && <p className="mt-1.5 max-w-[80ch] text-[13px] text-muted-foreground">{desc}</p>}</div>
      {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
    </header>
  );
}
export function Panel({ title, desc, action, children, className, flush }: { title?: ReactNode; desc?: ReactNode; action?: ReactNode; children: ReactNode; className?: string; flush?: boolean }) {
  return (
    <Card className={cn("min-w-0 gap-3", flush && "pb-2", className)}>
      {title && <CardHeader><CardTitle className="text-[15px]">{title}</CardTitle>{desc && <CardDescription className="text-xs">{desc}</CardDescription>}{action && <CardAction className="max-w-full max-sm:col-start-1 max-sm:row-start-3 max-sm:mt-2 max-sm:justify-self-start max-sm:overflow-x-auto">{action}</CardAction>}</CardHeader>}
      <CardContent className={cn(flush && "px-0")}>{children}</CardContent>
    </Card>
  );
}
export function Meter({ v, color }: { v: number | null | undefined; color?: string }) {
  const c = color ?? (v == null ? "var(--border)" : v < 0.15 ? "var(--bad)" : v < 0.4 ? "var(--warn)" : "var(--ok)");
  return <div className={cn("meter", v == null && "null")} style={{ ["--v" as string]: v ?? 1, ["--mc" as string]: c }}><i /></div>;
}
export function Legend({ items }: { items: { label: string; color: string }[] }) {
  return <div className="mt-3 flex flex-wrap gap-4 text-xs text-muted-foreground">{items.map((i) => <span key={i.label} className="inline-flex items-center gap-1.5"><i className="size-2.5 rounded-[3px]" style={{ background: i.color }} />{i.label}</span>)}</div>;
}
export function Count({ n }: { n: number | undefined }) { return <Badge variant="secondary" className="tabular-nums">{n ?? "—"}</Badge>; }

export function Empty({ icon, title, body, action }: { icon?: ReactNode; title: ReactNode; body?: ReactNode; action?: ReactNode }) {
  return (
    <div className="grid place-items-center gap-2 px-6 py-10 text-center">
      {icon && <div className="grid size-10 place-items-center rounded-xl bg-muted text-muted-foreground">{icon}</div>}
      <b className="text-sm">{title}</b>
      {body && <p className="max-w-[56ch] text-xs text-muted-foreground">{body}</p>}
      {action}
    </div>
  );
}

/**
 * Renders the three non-data states of a query honestly: loading (skeleton), feature not enabled in this
 * deployment (503 → explained empty state, never fabricated data), and other errors (with retry).
 */
export function Q<T>({ q, children, skeleton = 3, what }: { q: UseQueryResult<T>; children: (d: T) => ReactNode; skeleton?: number; what?: string }) {
  if (q.isPending) return <div className="grid gap-2">{Array.from({ length: skeleton }, (_, i) => <Skeleton key={i} className="h-9 w-full" />)}</div>;
  if (q.isError) {
    if (isUnavailable(q.error)) return <Empty icon={<PlugZap className="size-5" />} title={`${what ?? "这项数据"}在当前部署中未启用`} body="网关返回 503：对应的状态或存储没有接入。启用后这里会显示真实数据。" />;
    const e = describeError(q.error);
    return <Empty icon={<AlertTriangle className="size-5" />} title={e.title} body={e.detail} action={<Button size="sm" variant="outline" onClick={() => q.refetch()}><RefreshCw />重试</Button>} />;
  }
  return <>{children(q.data as T)}</>;
}

export function Since({ t }: { t: number | null | undefined }) { return <span className="text-xs text-muted-foreground" title={t ? new Date(t).toLocaleString() : undefined}>{fmt.ago(t)}</span>; }
