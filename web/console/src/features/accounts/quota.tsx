// Live quota windows (AccountQuotaObservation). Managed credentials expose them via credential metadata
// (`live_quota`), native Grok accounts via /admin/native-accounts/{id}/usage. Window names are shown as the
// provider reports them; the human label is derived from duration_seconds, never from list position.
import { useQueries, useQuery } from "@tanstack/react-query";
import { get, type S } from "@/api/hooks";
import { Meter, Mono } from "@/components/kit";
import { fmt } from "@/lib/format";

export type QuotaObs = S<"AccountQuotaObservation">;
export type Win = QuotaObs["windows"][number];
type Inv = S<"UnifiedAccountItem">;

export function windowLabel(w: Win) {
  const d = w.duration_seconds;
  if (d === 3600) return "每小时";
  if (d === 18000) return "5 小时";
  if (d === 86400) return "每日";
  if (d === 604800) return "每周";
  if (d && d >= 2419200 && d <= 2678400) return "每月";
  if (d) return d >= 86400 ? `${Math.round(d / 86400)} 天` : `${Math.round(d / 3600)} 小时`;
  return w.name;
}
export const remaining = (w: Win) => (w.used_percent == null ? (w.limit && w.used != null ? 1 - w.used / w.limit : null) : 1 - w.used_percent / 100);
export const tightest = (o: QuotaObs | null | undefined) => {
  const r = (o?.windows ?? []).map(remaining).filter((x): x is number => x != null);
  return r.length ? Math.min(...r) : null;
};

async function fetchQuota(a: Inv): Promise<{ obs: QuotaObs | null; error: string | null }> {
  if (a.native && a.native_account) {
    const r = await get<{ observation: QuotaObs | null; error: string | null }>("/admin/native-accounts/{account_id}/usage", { path: { account_id: a.id }, query: { revision: a.native_account.revision } });
    return { obs: r.observation, error: r.error };
  }
  const m = await get<S<"CredentialMetadata">>("/admin/credentials/{credential_id}/metadata", { path: { credential_id: a.id } });
  return { obs: m.live_quota ?? null, error: m.live_quota_error ?? null };
}
const quotaQuery = (a: Inv) => ({ queryKey: ["quota", a.id], queryFn: () => fetchQuota(a), staleTime: 60_000, retry: false, enabled: !!a.id });
export const useQuota = (a: Inv) => useQuery(quotaQuery(a));
export const useQuotas = (as: Inv[]) => useQueries({ queries: as.map(quotaQuery) });

const ERR: Record<string, string> = { not_implemented_or_not_connected: "该类账号不报告配额", not_connected: "未连接", unauthorized: "上游拒绝授权", forbidden: "上游拒绝访问", egress_denied: "出口策略拒绝", invalid_response: "上游响应无法解析", busy: "正在同步", unavailable: "暂不可用", configuration_changed: "配置已变化" };

export function QuotaWindows({ obs, error, compact, loading }: { obs: QuotaObs | null | undefined; error?: string | null; compact?: boolean; loading?: boolean }) {
  if (loading) return <span className="t-shimmer text-xs text-muted-foreground" data-text="正在读取配额…">正在读取配额…</span>;
  if (!obs || obs.windows.length === 0) return <span className="text-xs text-muted-foreground">{error ? ERR[error] ?? error : "上游未报告配额窗口"}</span>;
  const now = Date.now();
  return (
    <div className="grid grid-cols-2 gap-x-5 gap-y-2.5">
      {obs.windows.slice(0, compact ? 2 : 6).map((w) => {
        const r = remaining(w);
        return (
          <div key={w.name} className="min-w-0" title={`${w.name}${w.duration_seconds ? ` · ${w.duration_seconds}s` : ""}\n来源 ${obs.source}${obs.partial ? "（部分）" : ""} · 观测于 ${fmt.date(obs.observed_at_ms)}${w.limit != null ? `\n已用 ${fmt.n(w.used)} / ${fmt.n(w.limit)} ${w.unit ?? ""}` : ""}${w.reset_at_ms ? `\n重置于 ${fmt.date(w.reset_at_ms)}` : ""}`}>
            <div className="flex items-baseline gap-2 text-xs"><span className="flex-1 truncate font-medium text-muted-foreground">{windowLabel(w)}</span><span className="text-muted-foreground">{w.reset_at_ms ? fmt.dur(w.reset_at_ms - now) : ""}</span><b className="tabular-nums">{r == null ? "—" : fmt.pct(r, 0)}</b></div>
            <div className="mt-1"><Meter v={r} /></div>
            {!compact && <div className="mt-1 flex justify-between gap-2 text-[11px] text-muted-foreground"><Mono className="truncate">{w.name}</Mono><span className="shrink-0">{w.limit != null ? `${fmt.n(w.used)}/${fmt.n(w.limit)} ${w.unit ?? ""}` : "剩余比例"}</span></div>}
          </div>
        );
      })}
    </div>
  );
}
