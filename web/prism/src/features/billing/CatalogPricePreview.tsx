import { useId, useState } from "react";
import { compareCatalogEntries, formatRate, RATE_FIELDS, rateLabel, type CatalogEntry } from "./model";

const PAGE_SIZE = 50;

/** Pagination changes only the review window, never the frozen import payload. */
export function CatalogPricePreview({ entries, baseline, allowComparison = true }: Readonly<{
  entries: readonly CatalogEntry[];
  baseline: readonly CatalogEntry[];
  allowComparison?: boolean;
}>) {
  const [view, setView] = useState<"all" | "changes">("all");
  const [page, setPage] = useState(0);
  const listId = useId();
  const changes = compareCatalogEntries(baseline, entries);
  const count = view === "all" ? entries.length : changes.length;
  const pages = Math.max(1, Math.ceil(count / PAGE_SIZE));
  const current = Math.min(page, pages - 1);
  const start = current * PAGE_SIZE;
  const label = view === "all" ? "完整价格" : "价格差异";

  return <section className="catalog-price-preview" aria-label="核对目录价格">
    {allowComparison ? <div className="bill-actions" role="group" aria-label="价格预览内容">
      <button type="button" className="secondary" aria-pressed={view === "all"}
        onClick={() => { setView("all"); setPage(0); }}>完整价格（{entries.length}）</button>
      <button type="button" className="secondary" aria-pressed={view === "changes"}
        onClick={() => { setView("changes"); setPage(0); }}>价格差异（{changes.length}）</button>
    </div> : null}
    <div className="price-preview-list" id={listId} key={`${view}-${current}`}>
      {view === "all" ? entries.slice(start, start + PAGE_SIZE).map((entry, index) =>
        <details key={JSON.stringify([entry.provider_id, entry.channel_id, entry.model])}>
          <summary><strong className="mono">{entry.model}</strong><span className="entity-meta">第 {start + index + 1} 条 · {entry.provider_id} / {entry.channel_id}</span></summary>
          <dl className="fact-grid">{RATE_FIELDS.map(field => <div key={field}><dt>{rateLabel(field)}</dt><dd>{formatRate(entry[field])}</dd></div>)}</dl>
        </details>) : changes.slice(start, start + PAGE_SIZE).map(change => {
          const entry = change.after ?? change.before!;
          return <details key={JSON.stringify([entry.provider_id, entry.channel_id, entry.model])}>
            <summary><strong className="mono">{entry.model}</strong><span className="entity-meta">{entry.provider_id} / {entry.channel_id} · {change.kind === "removed" ? "新目录未包含" : change.kind === "added" ? "新增" : "费率变化"}</span></summary>
            <dl className="fact-grid">{RATE_FIELDS.map(field => <div key={field}><dt>{rateLabel(field)}</dt><dd>{change.before?.[field] ?? "—"} → {change.after?.[field] ?? "—"}</dd></div>)}</dl>
          </details>;
        })}
      {count === 0 ? <p className="empty-state">与对比目录的价格相同。</p> : null}
    </div>
    <nav className="bill-actions price-preview-pagination" aria-label="价格预览分页" aria-controls={listId}>
      <button type="button" className="secondary" disabled={current === 0} onClick={() => setPage(current - 1)}>上一页</button>
      <label>页码<select aria-label="价格预览页码" value={current} onChange={event => setPage(Number(event.target.value))}>
        {Array.from({ length: pages }, (_, index) => <option key={index} value={index}>第 {index + 1} / {pages} 页</option>)}
      </select></label>
      <button type="button" className="secondary" disabled={current + 1 === pages} onClick={() => setPage(current + 1)}>下一页</button>
      <span role="status">{label}：{count === 0 ? "0" : `${start + 1}–${Math.min(start + PAGE_SIZE, count)}`} / {count} 条</span>
    </nav>
  </section>;
}
