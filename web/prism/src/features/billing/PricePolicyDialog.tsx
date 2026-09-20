import { isCancelledError, useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore } from "../config-versions/versionStore";
import { isBillingOwner, runBillingPolicyTask, type BillingOwner, type BillingReceipt } from "./billingTask";
import { formatTime, isEffective, type Catalog, type PricePolicy } from "./model";

export type PricePolicyAction=Readonly<{kind:"bind"|"clear";owner:BillingOwner;baseline:PricePolicy|null;catalogs:readonly Catalog[]}>;

export function PricePolicyDialog({action,onClose,onDone,onReview}:Readonly<{
  action:PricePolicyAction;onClose:()=>void;onDone:(receipt:BillingReceipt)=>void;onReview:(receipt:BillingReceipt)=>Promise<void>;
}>) {
  const [selected,setSelected]=useState(action.baseline?.catalog_version_id??"");
  const [receipt,setReceipt]=useState<BillingReceipt>();
  const [error,setError]=useState<string>();
  const submitted=useRef(false);
  const formId="price-policy-form";
  const effective=action.catalogs.filter(row=>isEffective(row,Date.now()));
  useEffect(()=>{
    const closeIfLost=()=>{if(!isBillingOwner(action.owner))onClose();};
    const stopVersion=useVersionStore.subscribe(closeIfLost);
    const stopSession=useSessionStore.subscribe(closeIfLost);
    return ()=>{stopVersion();stopSession();};
  },[action.owner,onClose]);
  const save=useMutation({mutationFn:()=>runBillingPolicyTask(action.owner,action.baseline,action.kind==="clear"?{kind:"clear"}:{kind:"bind",catalogId:selected}),
    onSuccess:next=>{if(isBillingOwner(action.owner))setReceipt(next);},
    onError:cause=>{if(!isCancelledError(cause)&&isBillingOwner(action.owner)){submitted.current=false;setError(asAppError(cause).message);}},
  });
  const review=useMutation({mutationFn:async(next:BillingReceipt)=>onReview(next)});
  const submit=(event?:FormEvent)=>{
    event?.preventDefault();
    if(submitted.current||!isBillingOwner(action.owner)||(action.kind==="bind"&&(!selected||selected===action.baseline?.catalog_version_id)))return;
    submitted.current=true;setError(undefined);save.mutate();
  };
  const done=()=>{if(receipt?.kind==="unconfirmed")onClose();else if(receipt)onDone(receipt);else onClose();};
  const footer=receipt?.kind==="unconfirmed"?<><SheetDismissButton className="secondary" disabled={review.isPending} onDismiss={onClose}>稍后核对</SheetDismissButton><button type="button" disabled={review.isPending} onClick={()=>review.mutate(receipt)}>核对草稿</button></>
    :receipt?<SheetDismissButton onDismiss={done}>完成</SheetDismissButton>
    :<><SheetDismissButton className="secondary" disabled={save.isPending}>取消</SheetDismissButton><button type={action.kind==="bind"?"submit":"button"} form={action.kind==="bind"?formId:undefined} className={action.kind==="clear"?"danger":undefined} disabled={save.isPending||submitted.current||(action.kind==="bind"&&(!selected||selected===action.baseline?.catalog_version_id))} onClick={action.kind==="clear"?()=>submit():undefined}>{action.kind==="bind"?"保存到草稿":"确认清除"}</button></>;
  return <Sheet title={receipt?"价格策略结果":action.kind==="bind"?"绑定路由价格目录":"清除路由价格策略"}
    description={receipt?"此结果只涉及所选配置草稿；目录本身是全局资源。":action.kind==="bind"?"明确选择一份已生效目录；保存草稿后还需发布。":"清除仅影响所选草稿的路由价格比较；目录和账本保留。"}
    layout={receipt?"inspector":action.kind==="clear"?"confirm":"form"} tone={action.kind==="clear"&&!receipt?"danger":"default"}
    onEscape={done} busy={save.isPending||review.isPending} isDirty={!receipt&&action.kind==="bind"&&selected!==(action.baseline?.catalog_version_id??"")} footer={footer}>
    {receipt?<><p role={receipt.kind==="unconfirmed"?"alert":"status"}>{receipt.message}</p><p>目录：<strong className="mono">{receipt.target}</strong></p><p>工作配置 ID：<code className="mono">{receipt.workingVersion.id}</code></p>{review.isError?<p role="alert">重读失败：{asAppError(review.error).message}。可重试或按此 ID 稍后核对。</p>:null}</>
      :action.kind==="clear"?<p className="reveal-warning">当前绑定 <strong className="mono">{action.baseline?.catalog_version_id}</strong>。清除后，草稿中的路由价格比较将关闭；只有发布后才影响服务中的路由。</p>
      :<form id={formId} className="sheet-form" onSubmit={submit}><fieldset disabled={save.isPending||submitted.current}><label>已生效的价格目录<select required value={selected} onChange={event=>setSelected(event.target.value)}><option value="">请选择目录</option>{effective.map(row=><option key={row.catalog_version_id} value={row.catalog_version_id}>{row.entries[0]?.model??"空目录"}{row.entries.length>1?` 等 ${row.entries.length} 项`:""} · {formatTime(row.effective_at_ms)} · {row.source==="operator"?"运维录入":row.source==="imported"?"外部导入":row.source}</option>)}</select></label><p className="muted">选择目录不会导入或修改费率；保存后只改变当前草稿的比较策略。</p></fieldset></form>}
    {error?<p role="alert">{error}</p>:null}
  </Sheet>;
}
