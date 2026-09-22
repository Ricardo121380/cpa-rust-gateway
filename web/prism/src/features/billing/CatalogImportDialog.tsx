import { isCancelledError, useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { asAppError } from "../../api/errors";
import { InlineWorkspace } from "../../components/InlineWorkspace";
import { useOperationBoundary } from "../../components/OperationBoundary";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore } from "../config-versions/versionStore";
import { isBillingOwner, runBillingCatalogTask, type BillingOwner, type BillingReceipt } from "./billingTask";
import { PriceEntriesEditor } from "./PriceEntriesEditor";
import { CatalogPricePreview } from "./CatalogPricePreview";
import { compareCatalogEntries, formatCatalogEntries, formatTime, parseCatalogEntries, sourceLabel, validBillingText, validBillingTime, WRITABLE_SOURCES, type Catalog, type CatalogEntry } from "./model";

const catalogLabel=(catalog:Catalog)=>`${catalog.entries[0]?.model??"空目录"}${catalog.entries.length>1?` 等 ${catalog.entries.length} 项`:""} · ${formatTime(catalog.effective_at_ms)}`;

type Prepared=Readonly<{id:string;effectiveAt:number;source:string;entries:readonly CatalogEntry[];baseline?:Catalog}>;
export type CatalogImportAction=Readonly<{owner:BillingOwner;catalogs:readonly Catalog[];template?:Catalog}>;

export function CatalogImportDialog({action,onClose,onDone,onReview}:Readonly<{
  action:CatalogImportAction;onClose:()=>void;onDone:(receipt:BillingReceipt)=>void;onReview:(receipt:BillingReceipt)=>Promise<void>;
}>) {
  const boundary=useOperationBoundary();
  const [id,setId]=useState(()=>`catalog-${crypto.randomUUID()}`);
  const [effective,setEffective]=useState("");
  const [source,setSource]=useState<string>("operator");
  const [baselineId,setBaselineId]=useState(action.template?.catalog_version_id??"");
  const [dirty,setDirty]=useState(false);
  const [quotePending,setQuotePending]=useState(false);
  const [prepared,setPrepared]=useState<Prepared>();
  const [receipt,setReceipt]=useState<BillingReceipt>();
  const [error,setError]=useState<string>();
  const submitted=useRef(false);
  const formId="billing-catalog-import-form";
  useEffect(()=>{
    const closeIfLost=()=>{if(!isBillingOwner(action.owner))onClose();};
    const stopVersion=useVersionStore.subscribe(closeIfLost);
    const stopSession=useSessionStore.subscribe(closeIfLost);
    return ()=>{stopVersion();stopSession();};
  },[action.owner,onClose]);
  const save=useMutation({mutationFn:async(input:Prepared)=>runBillingCatalogTask(action.owner,{kind:"import",target:input.id,effectiveAt:input.effectiveAt,source:input.source,entries:input.entries}),
    onSuccess:next=>{if(isBillingOwner(action.owner))setReceipt(next);},
    onError:cause=>{if(!isCancelledError(cause)&&isBillingOwner(action.owner)){submitted.current=false;setError(asAppError(cause).message);}},
  });
  const review=useMutation({mutationFn:async(next:BillingReceipt)=>onReview(next)});
  const prepare=(event:FormEvent<HTMLFormElement>)=>{
    event.preventDefault();
    if(!isBillingOwner(action.owner))return;
    if(quotePending){setError("价格来源仍在读取；请等待结果后再预览。");return;}
    const entries=parseCatalogEntries(String(new FormData(event.currentTarget).get("entries")??""));
    if(!entries.ok){setError(entries.reason);return;}
    if(!validBillingText(id,128)){setError("目录标识须为非空且不超过 128 字节，不含首尾空白或控制字符。");return;}
    if(!WRITABLE_SOURCES.some(value=>value===source)){setError("请选择可写入的目录来源。");return;}
    const effectiveAt=Date.parse(effective);
    if(!validBillingTime(effectiveAt)){setError("生效时间无效，请重新选择。");return;}
    if(action.catalogs.some(row=>row.catalog_version_id===id)){setError("目录标识已存在；请使用新的目录版本。");return;}
    setError(undefined);
    setPrepared({id,effectiveAt,source,entries:entries.entries,baseline:action.catalogs.find(row=>row.catalog_version_id===baselineId)});
  };
  const commit=()=>{if(!prepared||submitted.current||!isBillingOwner(action.owner))return;submitted.current=true;setError(undefined);save.mutate(prepared);};
  const done=()=>{if(receipt?.kind==="unconfirmed")onClose();else if(receipt)onDone(receipt);else onClose();};
  const changes=prepared?compareCatalogEntries(prepared.baseline?.entries??[],prepared.entries):[];
  const footer=receipt?.kind==="unconfirmed"?<><button type="button" className="secondary" disabled={review.isPending} onClick={()=>boundary.request(onClose)}>稍后核对</button><button type="button" disabled={review.isPending} onClick={()=>review.mutate(receipt)}>核对工作配置</button></>
    :receipt?<button type="button" onClick={done}>{receipt.kind==="catalog_unapplied"?"查看工作草稿":"完成"}</button>
    :prepared?<><button type="button" className="secondary" disabled={save.isPending} onClick={()=>{setPrepared(undefined);setError(undefined);}}>返回修改</button><button type="button" disabled={save.isPending||submitted.current} onClick={commit}>确认导入</button></>
    :<><button type="button" className="secondary" onClick={()=>boundary.request(onClose)}>取消</button><button type="submit" form={formId} disabled={quotePending}>预览差异</button></>;
  return <InlineWorkspace title={receipt?"目录导入结果":prepared?"确认完整价格目录":action.template?"复制现有目录并调整":"导入价格目录"}
    description={receipt?"全局目录的写入与工作配置的发布分别显示。":prepared?"此目录会新增到全局价格历史；不会自动绑定路由策略。":"填写整份价目表；新目录对所有配置可见，历史目录和账本保留。"}
    onClose={done} busy={save.isPending||review.isPending} dirty={!receipt&&(dirty||prepared!==undefined)} footer={footer}>
    {receipt?<><p role={receipt.kind==="unconfirmed"?"alert":"status"}>{receipt.message}</p><p>目标目录：<strong className="mono">{receipt.target}</strong></p><p>工作配置 ID：<code className="mono">{receipt.workingVersion.id}</code></p>{receipt.catalog?<p>服务端回执：{receipt.catalog.entry_count} 条 · 生效 {formatTime(receipt.catalog.effective_at_ms)}</p>:null}{receipt.observedCatalog?<p>目录重读可见；这不证明原请求回执已确认。</p>:null}{review.isError?<p role="alert">重读失败：{asAppError(review.error).message}。可重试或按此配置 ID 稍后核对。</p>:null}</>
      :<><form id={formId} className="sheet-form" hidden={prepared!==undefined} onSubmit={prepare}><fieldset className="workflow-section" disabled={save.isPending||submitted.current}><legend>目录与生效时间</legend>
        <label>生效时间（本地时区）<input type="datetime-local" required value={effective} onChange={event=>{setDirty(true);setEffective(event.target.value);}}/></label>
        <details><summary>目录信息与对比基线</summary><label>目录版本 ID<input className="mono" required maxLength={128} value={id} onChange={event=>{setDirty(true);setId(event.target.value);}}/></label><label>来源<select value={source} onChange={event=>{setDirty(true);setSource(event.target.value);}}>{WRITABLE_SOURCES.map(value=><option key={value} value={value}>{sourceLabel(value)}</option>)}</select></label><label>与现有目录对比<select value={baselineId} onChange={event=>{setDirty(true);setBaselineId(event.target.value);}}><option value="">不比较</option>{action.catalogs.map(row=><option key={row.catalog_version_id} value={row.catalog_version_id}>{catalogLabel(row)}</option>)}</select></label></details>
      </fieldset><PriceEntriesEditor initial={action.template?formatCatalogEntries(action.template.entries):undefined} disabled={save.isPending||submitted.current||prepared!==undefined} onDirty={()=>setDirty(true)} onBusyChange={setQuotePending}/>
      </form>
      {prepared?<div className="price-import-preview">
        <p><strong>新价格目录</strong> · {sourceLabel(prepared.source)} · {prepared.entries.length} 条</p><details><summary>目录技术标识</summary><code>{prepared.id}</code></details>
        <p>生效时刻（UTC）：{formatTime(prepared.effectiveAt)}</p>
        <dl className="price-diff-summary"><div><dt>新增</dt><dd>{changes.filter(row=>row.kind==="added").length}</dd></div><div><dt>费率变化</dt><dd>{changes.filter(row=>row.kind==="changed").length}</dd></div><div><dt>新目录未包含</dt><dd>{changes.filter(row=>row.kind==="removed").length}</dd></div></dl><p className="muted">比较基线：{prepared.baseline?catalogLabel(prepared.baseline):"空目录"}</p>
        <p className="muted">未包含的条目不会删除历史目录，也不一定停止已有计价；服务按有效目录中的精确元组查找。</p>
        <CatalogPricePreview entries={prepared.entries} baseline={prepared.baseline?.entries??[]}/>
      </div>:null}</>}
    {error?<p role="alert">{error}</p>:null}
  </InlineWorkspace>;
}
