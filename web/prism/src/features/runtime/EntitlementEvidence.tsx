import { formatObservedAt, type ProviderAccountEntitlement } from "./model";

export function EntitlementEvidence({ entitlement }: Readonly<{ entitlement: ProviderAccountEntitlement | null }>) {
  if (entitlement === null) return <span className="muted">权益未观测</span>;
  return (
    <details>
      <summary>{entitlement.domain} · {entitlement.tier}</summary>
      <dl>
        <dt>权益域</dt><dd>{entitlement.domain}</dd>
        <dt>套餐</dt><dd>{entitlement.tier}</dd>
        <dt>来源</dt><dd>{entitlement.source}</dd>
        <dt>置信度</dt><dd>{entitlement.confidence}</dd>
        <dt>观测时间</dt><dd>{formatObservedAt(entitlement.observed_at_ms)}</dd>
      </dl>
    </details>
  );
}
