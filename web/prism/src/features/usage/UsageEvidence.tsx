export type UsageProvenance = "measured" | "estimated" | "unknown";
export type InputAccounting = "inclusive" | "exclusive" | "unknown";

export function provenanceLabel(value: UsageProvenance | "mixed" | undefined): string {
  if (value === "measured") return "上游实测";
  if (value === "estimated") return "估算用量";
  if (value === "mixed") return "混合来源";
  return "来源未知";
}

export function UsageEvidence({ provenance, inputAccounting }: {
  provenance?: UsageProvenance | "mixed"; inputAccounting?: InputAccounting;
}) {
  const accounting = inputAccounting === "inclusive" ? "输入已包含缓存 token"
    : inputAccounting === "exclusive" ? "输入不包含缓存 token" : "输入与缓存关系未知";
  return <span className="small muted">{provenanceLabel(provenance)} · {accounting}</span>;
}
