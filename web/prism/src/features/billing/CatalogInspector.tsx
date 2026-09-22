import { CatalogPricePreview } from "./CatalogPricePreview";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { formatTime, sourceLabel, type Catalog } from "./model";

export function CatalogInspector({catalog,onClose}:Readonly<{catalog:Catalog;onClose:()=>void}>) {

  return <Sheet title="价格目录详情" description="全局、只读的完整目录；历史条目不会在此修改。" layout="inspector" onEscape={onClose} footer={<SheetDismissButton onDismiss={onClose}>关闭</SheetDismissButton>}>
    <dl className="fact-grid"><div><dt>目录模型</dt><dd>{catalog.entries[0]?.model??"空目录"}{catalog.entries.length>1?` 等 ${catalog.entries.length} 项`:""}</dd></div><div><dt>来源</dt><dd>{sourceLabel(catalog.source)}</dd></div><div><dt>生效时间（UTC）</dt><dd>{formatTime(catalog.effective_at_ms)}</dd></div><div><dt>创建时间（UTC）</dt><dd>{formatTime(catalog.created_at_ms)}</dd></div><div><dt>完整条目</dt><dd>{catalog.entries.length}</dd></div></dl>
    <details><summary>目录技术标识</summary><code>{catalog.catalog_version_id}</code></details>
    <CatalogPricePreview entries={catalog.entries} baseline={[]} allowComparison={false}/>
  </Sheet>;
}
