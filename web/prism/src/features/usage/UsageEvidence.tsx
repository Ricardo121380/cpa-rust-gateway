import { useMessages, type Pack } from "../../i18n/messages";

export type UsageProvenance = "measured" | "estimated" | "unknown";
export type InputAccounting = "inclusive" | "exclusive" | "unknown";

export function provenanceLabel(value: UsageProvenance | "mixed" | undefined, labels: Pack["usageEvidence"]): string {
  return labels[value ?? "unknown"];
}

export function UsageEvidence({ provenance, inputAccounting }: {
  provenance?: UsageProvenance | "mixed"; inputAccounting?: InputAccounting;
}) {
  const labels = useMessages().usageEvidence;
  const accounting = inputAccounting === "inclusive" ? labels.inclusive
    : inputAccounting === "exclusive" ? labels.exclusive : labels.accountingUnknown;
  return <span className="small muted">{provenanceLabel(provenance, labels)} · {accounting}</span>;
}
