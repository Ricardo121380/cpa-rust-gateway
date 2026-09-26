import { capacityWarnings, type GatewayCounters } from "./metrics";

function bytes(value: number | null): string {
  if (value === null) return "未观测";
  return value >= 1024 ** 3 ? `${(value / 1024 ** 3).toFixed(2)} GiB` : `${(value / 1024 ** 2).toFixed(1)} MiB`;
}

export function StorageCapacity({ capacity }: Readonly<{ capacity: GatewayCounters["capacity"] }>) {
  const warnings = capacityWarnings(capacity, Date.now());
  return <section className="card" aria-label="存储与记录容量">
    <h3>存储与记录容量</h3>
    {warnings.length > 0 ? <ul role="status">{warnings.map(warning => <li key={warning}>{warning}</li>)}</ul> : null}
    <dl className="fact-grid">
      <div><dt>磁盘可用 / 总量</dt><dd>{bytes(capacity.availableBytes)} / {bytes(capacity.totalBytes)}</dd></div>
      <div><dt>数据库 / WAL</dt><dd>{bytes(capacity.databaseBytes)} / {bytes(capacity.walBytes)}</dd></div>
      <div><dt>记录队列占用 / 容量</dt><dd>{capacity.queueUsed ?? "未观测"} / {capacity.queueCapacity ?? "未观测"}</dd></div>
      <div><dt>容量观测时间</dt><dd>{capacity.observedAt === null ? "未观测" : new Date(capacity.observedAt).toLocaleString()}</dd></div>
    </dl>
    <p className="stat-sub">容量每分钟采样；告警不会自动删除历史。队列占用不包含已取出、等待写入的批次。</p>
  </section>;
}
