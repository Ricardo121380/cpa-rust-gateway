// Formatting helpers. All times render in the viewer's local time zone.
export const fmt = {
  n(v: number | null | undefined) {
    if (v == null) return "—";
    const a = Math.abs(v);
    if (a >= 1e9) return (v / 1e9).toFixed(2) + "B";
    if (a >= 1e6) return (v / 1e6).toFixed(2) + "M";
    if (a >= 1e4) return (v / 1e3).toFixed(1) + "K";
    return Math.round(v).toLocaleString("en-US");
  },
  int: (v: number | null | undefined) => (v == null ? "—" : Math.round(v).toLocaleString("en-US")),
  pct: (v: number | null | undefined, d = 1) => (v == null || Number.isNaN(v) ? "—" : (v * 100).toFixed(d) + "%"),
  ms(v: number | null | undefined) {
    if (v == null) return "—";
    return v >= 1000 ? (v / 1000).toFixed(v >= 1e4 ? 1 : 2) + "s" : Math.round(v) + "ms";
  },
  /** microunits → currency string; null stays an honest dash. */
  money(micro: number | null | undefined) {
    if (micro == null) return "—";
    const v = micro / 1e6;
    return "$" + v.toLocaleString("en-US", { minimumFractionDigits: v < 10 ? 4 : 2, maximumFractionDigits: v < 10 ? 4 : 2 });
  },
  dur(ms: number) {
    if (ms <= 0) return "已到期";
    const d = Math.floor(ms / 864e5), h = Math.floor((ms % 864e5) / 36e5), m = Math.floor((ms % 36e5) / 6e4);
    return d ? `${d}天${h}时` : h ? `${h}时${m}分` : `${Math.max(m, 1)}分`;
  },
  ago(t: number | null | undefined, now = Date.now()) {
    if (t == null) return "—";
    const d = now - t;
    if (d < 6e4) return "刚刚";
    if (d < 36e5) return Math.floor(d / 6e4) + " 分钟前";
    if (d < 864e5) return Math.floor(d / 36e5) + " 小时前";
    return Math.floor(d / 864e5) + " 天前";
  },
  time: (t: number) => new Date(t).toLocaleTimeString("zh-CN", { hour12: false }),
  date: (t: number) => new Date(t).toLocaleString("zh-CN", { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit", hour12: false }),
  day: (t: number) => new Date(t).toLocaleDateString("zh-CN", { month: "numeric", day: "numeric" }),
  uptime(s: number) { const d = Math.floor(s / 86400), h = Math.floor((s % 86400) / 3600), m = Math.floor((s % 3600) / 60); return d ? `${d} 天 ${h} 小时` : h ? `${h} 小时 ${m} 分` : `${m} 分`; },
};

export const sum = <T,>(a: readonly T[], f: (x: T) => number | null | undefined) => a.reduce((s, x) => s + (f(x) ?? 0), 0);
