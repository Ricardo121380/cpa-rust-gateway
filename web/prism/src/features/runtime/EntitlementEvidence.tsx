import { formatObservedAt, type ProviderAccountEntitlement } from "./model";
import { isKnownEntitlement } from "./entitlements";

export function EntitlementEvidence({ entitlement, expanded = false }: Readonly<{ entitlement: ProviderAccountEntitlement | null; expanded?: boolean }>) {
  if (entitlement === null) return <span className="muted">权益未观测</span>;
  return (
    <details open={expanded}>
      <summary>{entitlement.domain} · {entitlement.tier}</summary>
      {isKnownEntitlement(entitlement) ? null : <p className="small muted">未识别的权益组合，以下保留服务端原值。</p>}
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
