import { WorkspaceTabs } from "../../app/WorkspaceTabs";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { flushSync } from "react-dom";
import { useNavigate } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { useOperationBoundary } from "../../components/OperationBoundary";
import { ReadStatus } from "../../components/ReadStatus";
import { useMessages } from "../../i18n/messages";
import { useVersionStore } from "../config-versions/versionStore";
import { CatalogImportDialog, type CatalogImportAction } from "./CatalogImportDialog";
import { CatalogPricePreview } from "./CatalogPricePreview";
import { CatalogInspector } from "./CatalogInspector";
import { CatalogRestoreDialog, type CatalogRestoreAction } from "./CatalogRestoreDialog";
import { ProcessingStatus } from "./ProcessingStatus";
import { PricePolicyDialog, type PricePolicyAction } from "./PricePolicyDialog";
import { captureBillingOwner, reviewBillingVersion, type BillingOwner, type BillingReceipt } from "./billingTask";
import { MAX_CATALOGS, formatCount, formatTime, isEffective, isPolicyUnset, sortCatalogs, sourceLabel, type Catalog, type PricePolicy } from "./model";
import "./billing.css";

type Action=
  |Readonly<{kind:"inspect";catalog:Catalog}>
  |Readonly<{kind:"import";data:CatalogImportAction}>
  |Readonly<{kind:"restore";data:CatalogRestoreAction}>
  |Readonly<{kind:"policy";data:PricePolicyAction}>;

export function BillingPage(){
  const boundary=useOperationBoundary();
  const t=useMessages();
  const navigate=useNavigate();
  const queryClient=useQueryClient();
  const context=useVersionStore(state=>state.context);
  const scope=context?.configVersionId;
  const revision=context?.revision;
  const [action,setAction]=useState<Action>();
  const [expanded,setExpanded]=useState<string>();
  const [notice,setNotice]=useState<string>();
  const [actionError,setActionError]=useState<string>();
  const catalogs=useQuery({queryKey:["billing-catalogs",scope,revision],queryFn:()=>call<readonly Catalog[]>("listBillingCatalogs",{},{versionScoped:true}),enabled:scope!==undefined,retry:false});
  const policy=useQuery({queryKey:["price-policy",scope,revision],queryFn:()=>call<PricePolicy>("getRoutingPricePolicy",{},{versionScoped:true}),enabled:scope!==undefined,retry:false});
  const policyUnset=policy.isError&&isPolicyUnset(policy.error);
  const policyKnown=policyUnset||(policy.isSuccess&&policy.data!==undefined);
  const catalogsReady=catalogs.data!==undefined&&!catalogs.isError;
  const rows=sortCatalogs(catalogs.data??[]);

  const openImport=(template?:Catalog)=>{
    try{if(!catalogsReady)throw new Error("价格目录尚未完整读取，请先重新读取。");setAction({kind:"import",data:{owner:captureBillingOwner(),catalogs:catalogs.data,template}});setActionError(undefined);}
    catch(cause){setActionError(asAppError(cause).message);}
  };
  const openRestore=(predecessor:Catalog)=>{
    try{if(!catalogsReady)throw new Error("价格目录读取失败，请重新读取后恢复。");setAction({kind:"restore",data:{owner:captureBillingOwner(),predecessor}});setActionError(undefined);}
    catch(cause){setActionError(asAppError(cause).message);}
  };
  const openPolicy=(kind:"bind"|"clear")=>{
    try{
      if(context?.status==="archived")throw new Error("历史配置不可修改。");
      if(!policyKnown||!catalogsReady)throw new Error("价格策略或目录尚未完整读取，请重新读取后操作。");
      if(kind==="clear"&&policyUnset)throw new Error("当前草稿尚未配置价格策略。");
      setAction({kind:"policy",data:{kind,owner:captureBillingOwner(),baseline:policyUnset?null:policy.data??null,catalogs:catalogs.data}});
      setActionError(undefined);
    }catch(cause){setActionError(asAppError(cause).message);}
  };
  const refresh=()=>{void queryClient.resetQueries({queryKey:["billing-catalogs"]});void queryClient.resetQueries({queryKey:["price-policy"]});};
  const finish=(receipt:BillingReceipt)=>{
    flushSync(()=>setAction(undefined));
    const store=useVersionStore.getState();
    if(store.context?.configVersionId===receipt.workingVersion.id&&store.context.status===receipt.workingVersion.status)store.advanceFromEtag(receipt.workingVersion.revision);
    else store.select(receipt.workingVersion);
    refresh();setNotice(receipt.message);
    if(receipt.kind==="catalog_unapplied")navigate("/versions");
  };
  const review=async(owner:BillingOwner,receipt:BillingReceipt)=>{
    const version=await reviewBillingVersion(owner,receipt);
    flushSync(()=>setAction(undefined));
    useVersionStore.getState().select(version);
    refresh();navigate("/versions");
  };

  if(scope===undefined)return <section className="billing-page"><h2>{t.nav.billing}</h2><ProcessingStatus compact/><div className="card empty-state"><p>选择配置后查看价格目录；计费处理状态跨版本可读。</p></div></section>;
  return <section className="billing-page">
    <header className="page-head"><h2>{t.nav.billing}</h2></header>
      <WorkspaceTabs />
    {action?.kind==="import"?<CatalogImportDialog action={action.data} onClose={()=>setAction(undefined)} onDone={finish} onReview={receipt=>review(action.data.owner,receipt)}/>:null}
    <ProcessingStatus compact/>
    {notice?<p className="action-notice" role="status">{notice} <button type="button" onClick={()=>setNotice(undefined)}>知道了</button></p>:null}
    {actionError?<p className="action-error" role="alert">{actionError} <button type="button" onClick={()=>setActionError(undefined)}>清除</button></p>:null}
    <details className="card bill-policy-disclosure"><summary>高级路由价格策略</summary><div className="bill-policy">
      <header className="page-head"><h3>路由价格比较</h3></header>
      <p className="bill-note">目录是全局历史；此处的绑定只属于工作配置草稿；保存后统一校验并应用，才用于路由比较。</p>
      {policy.isPending?<p role="status">正在读取价格策略…</p>:policyUnset?<p className="empty-state">当前版本未配置价格策略。此时路由价格证据为 disabled；这是正常的未配置状态。</p>:policy.isError?<p role="alert" className="action-error">{asAppError(policy.error).message} <button type="button" onClick={()=>void policy.refetch()}>重新读取</button></p>:<dl className="fact-grid"><div><dt>已绑定目录</dt><dd>{(()=>{const selected=rows.find(row=>row.catalog_version_id===policy.data?.catalog_version_id);return selected?`${selected.entries[0]?.model??sourceLabel(selected.source)} · ${formatTime(selected.effective_at_ms)}`:policy.data?.catalog_version_id;})()}</dd></div><div><dt>比较方式</dt><dd>按费率比较</dd></div></dl>}
      <div className="bill-actions"><button type="button" disabled={context?.status==="archived"||!policyKnown||!catalogsReady||!rows.some(row=>isEffective(row,Date.now()))} onClick={()=>boundary.request(()=>openPolicy("bind"))}>{policyUnset?"设置策略":"更换目录"}</button>{policyUnset?null:<button type="button" className="danger" disabled={context?.status==="archived"||!policyKnown} onClick={()=>boundary.request(()=>openPolicy("clear"))}>清除策略</button>}</div>
      {!rows.some(row=>isEffective(row,Date.now()))?<p className="muted">目前没有已生效且可绑定的目录。</p>:null}
    </div></details>
    <section className="card bill-catalogs" aria-label="全局价格目录"><header className="page-head"><h3>价格目录 <span className="idchip mono">{catalogs.data?formatCount(rows.length):"—"}</span></h3><button type="button" disabled={context?.status==="archived"||!catalogsReady||rows.length>=MAX_CATALOGS} onClick={()=>boundary.request(()=>openImport())}>导入目录</button></header>
      <p className="bill-note">导入整份目录会全局新增历史版本，对所有配置可见；不会自动绑定路由策略或修改历史账本。</p>
      {rows.length>=MAX_CATALOGS?<p role="status">价格目录已达到 {MAX_CATALOGS} 份上限，无法继续导入或恢复；现有目录保持可读。</p>:null}
      <ReadStatus pending={catalogs.isPending} error={catalogs.error} hasData={catalogs.data!==undefined} retry={()=>void catalogs.refetch()}/>
      {catalogs.data?.length===0?<p className="empty-state">还没有价格目录；历史用量不能据此推断为零费用。</p>:null}
      {rows.length?<div className="tablewrap"><table className="bill-table bill-catalog-table"><thead><tr><th>目录</th><th>生效时间（UTC）</th><th>来源</th><th>条目</th><th>操作</th></tr></thead><tbody>{rows.map(catalog=><tr key={catalog.catalog_version_id} data-resource-id={catalog.catalog_version_id}>
        <td data-label="目录"><strong className="mono">{catalog.entries[0]?.model??"空目录"}{catalog.entries.length>1?` 等 ${catalog.entries.length} 项`:""}</strong><small className="entity-meta">创建 {formatTime(catalog.created_at_ms)}</small>{isEffective(catalog,Date.now())?null:<span className="bill-future">未生效</span>}</td><td data-label="生效时间">{formatTime(catalog.effective_at_ms)}</td><td data-label="来源">{sourceLabel(catalog.source)}</td><td data-label="条目">{formatCount(catalog.entries.length)}</td>
        <td data-label="操作" className="row-actions"><button type="button" className="secondary" onClick={()=>boundary.request(()=>setAction({kind:"inspect",catalog}))}>详情</button><button type="button" className="secondary" onClick={()=>setExpanded(expanded===catalog.catalog_version_id?undefined:catalog.catalog_version_id)}>{expanded===catalog.catalog_version_id?"收起条目":"看条目"}</button><button type="button" className="secondary" disabled={context?.status==="archived"||!catalogsReady||rows.length>=MAX_CATALOGS} onClick={()=>boundary.request(()=>openImport(catalog))}>复制编辑</button><button type="button" className="secondary" disabled={context?.status==="archived"||!catalogsReady||rows.length>=MAX_CATALOGS} onClick={()=>boundary.request(()=>openRestore(catalog))}>恢复价格</button></td>
      </tr>)}</tbody></table></div>:null}
      {expanded?<div className="bill-entries-view"><h4>目录价格条目 · microunits / 百万 token</h4><CatalogPricePreview key={expanded} entries={rows.find(row=>row.catalog_version_id===expanded)?.entries??[]} baseline={[]} allowComparison={false}/></div>:null}
    </section>
    {action?.kind==="inspect"?<CatalogInspector catalog={action.catalog} onClose={()=>setAction(undefined)}/>:null}
    {action?.kind==="restore"?<CatalogRestoreDialog action={action.data} onClose={()=>setAction(undefined)} onDone={finish} onReview={receipt=>review(action.data.owner,receipt)}/>:null}
    {action?.kind==="policy"?<PricePolicyDialog action={action.data} onClose={()=>setAction(undefined)} onDone={finish} onReview={receipt=>review(action.data.owner,receipt)}/>:null}
  </section>;
}
