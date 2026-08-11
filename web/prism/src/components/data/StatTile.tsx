// Stat tile per dataviz spec: big tabular number, quiet sub-line, optional
// recessive sparkline. Text wears ink tokens; color never carries meaning
// alone here.
import { SparkLine } from "./SparkLine";

export function formatCount(value: number): string {
  if (value >= 1_000_000_000) return `${(value / 1_000_000_000).toFixed(1)}B`;
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 10_000) return `${(value / 1_000).toFixed(1)}K`;
  return String(value);
}

export function formatLatency(ms: number | null | undefined): string {
  if (ms === null || ms === undefined) return "—";
  return ms >= 1000 ? `${(ms / 1000).toFixed(1)}s` : `${Math.round(ms)}ms`;
}

export function StatTile({
  label,
  value,
  sub,
  spark,
}: Readonly<{
  label: string;
  value: string;
  sub?: string | undefined;
  spark?: readonly number[] | undefined;
}>) {
  return (
    <div className="stat-tile card">
      <span className="stat-label">{label}</span>
      <span className="stat-value mono">{value}</span>
      {sub !== undefined ? <span className="stat-sub">{sub}</span> : null}
      {spark !== undefined && spark.length > 1 ? <SparkLine values={spark} /> : null}
    </div>
  );
}
