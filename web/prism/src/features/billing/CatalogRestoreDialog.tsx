import { isCancelledError, useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore } from "../config-versions/versionStore";
import { isBillingOwner, runBillingCatalogTask, type BillingOwner, type BillingReceipt } from "./billingTask";
import { formatTime, validBillingText, validBillingTime, type Catalog } from "./model";

export type CatalogRestoreAction=Readonly<{owner:BillingOwner;predecessor:Catalog}>;

export function CatalogRestoreDialog({action,onClose,onDone,onReview}:Readonly<{
  action:CatalogRestoreAction;onClose:()=>void;onDone:(receipt:BillingReceipt)=>void;onReview:(receipt:BillingReceipt)=>Promise<void>;
}>) {
  const [id,setId]=useState(()=>`catalog-${crypto.randomUUID()}`);
  const [effective,setEffective]=useState("");
  const [prepared,setPrepared]=useState<{id:string;effectiveAt:number}>();
  const [receipt,setReceipt]=useState<BillingReceipt>();
  const [error,setError]=useState<string>();
  const submitted=useRef(false);
  const formId="billing-catalog-restore-form";
  useEffect(()=>{
    const closeIfLost=()=>{if(!isBillingOwner(action.owner))onClose();};
    const stopVersion=useVersionStore.subscribe(closeIfLost);
    const stopSession=useSessionStore.subscribe(closeIfLost);
    return ()=>{stopVersion();stopSession();};
  },[action.owner,onClose]);
  const save=useMutation({mutationFn:async(input:{id:string;effectiveAt:number})=>runBillingCatalogTask(action.owner,{kind:"restore",target:input.id,effectiveAt:input.effectiveAt,predecessor:action.predecessor}),
    onSuccess:next=>{if(isBillingOwner(action.owner))setReceipt(next);},
    onError:cause=>{if(!isCancelledError(cause)&&isBillingOwner(action.owner)){submitted.current=false;setError(asAppError(cause).message);}},
  });
  const review=useMutation({mutationFn:async(next:BillingReceipt)=>onReview(next)});
  const prepare=(event:FormEvent)=>{
    event.preventDefault();
    if(!validBillingText(id,128)){setError("新目录标识须为非空且不超过 128 字节，不含首尾空白或控制字符。");return;}
    if(id===action.predecessor.catalog_version_id){setError("新目录必须使用不同的标识。");return;}
    const effectiveAt=Date.parse(effective);
    if(!validBillingTime(effectiveAt)){setError("请选择有效的生效时间。");return;}
    setError(undefined);setPrepared({id,effectiveAt});
  };
  const commit=()=>{if(!prepared||submitted.current||!isBillingOwner(action.owner))return;submitted.current=true;setError(undefined);save.mutate(prepared);};
  const done=()=>{if(receipt?.kind==="unconfirmed")onClose();else if(receipt)onDone(receipt);else onClose();};
  const footer=receipt?.kind==="unconfirmed"?<><SheetDismissButton className="secondary" disabled={review.isPending} onDismiss={onClose}>稍后核对</SheetDismissButton><button type="button" disabled={review.isPending} onClick={()=>review.mutate(receipt)}>核对工作配置</button></>
    :receipt?<SheetDismissButton onDismiss={done}>{receipt.kind==="catalog_unapplied"?"查看工作草稿":"完成"}</SheetDismissButton>
    :prepared?<><button type="button" className="secondary" disabled={save.isPending} onClick={()=>setPrepared(undefined)}>返回修改</button><button type="button" disabled={save.isPending||submitted.current} onClick={commit}>创建新目录</button></>
    :<><SheetDismissButton className="secondary">取消</SheetDismissButton><button type="submit" form={formId}>核对恢复内容</button></>;
  return <Sheet title={receipt?"价格恢复结果":prepared?"确认创建恢复目录":"从历史目录恢复价格"}
    description={receipt?"新目录写入与配置应用分别显示。":"复制历史目录的全部条目，创建新的全局目录；旧目录与账本原样保留。"}
    layout={prepared||receipt?"confirm":"form"} onEscape={done} busy={save.isPending||review.isPending} isDirty={!receipt&&(!!effective||prepared!==undefined)} guardUnsaved={!receipt} footer={footer}>
    {receipt?<><p role={receipt.kind==="unconfirmed"?"alert":"status"}>{receipt.message}</p><p>来源 <strong className="mono">{action.predecessor.catalog_version_id}</strong> → 新目录 <strong className="mono">{receipt.target}</strong></p><p>工作配置 ID：<code className="mono">{receipt.workingVersion.id}</code></p>{review.isError?<p role="alert">重读失败：{asAppError(review.error).message}。可重试或按此配置 ID 稍后核对。</p>:null}</>
      :prepared?<div className="reveal-warning"><p>将 <strong className="mono">{action.predecessor.catalog_version_id}</strong> 的 {action.predecessor.entries.length} 条价格复制为 <strong className="mono">{prepared.id}</strong>。</p><p>新目录生效时刻（UTC）：{formatTime(prepared.effectiveAt)}。这不会回滚配置、删除中间目录或改写账本。</p></div>
      :<form id={formId} className="sheet-form" onSubmit={prepare}><fieldset disabled={save.isPending}><p>来源目录：<strong className="mono">{action.predecessor.catalog_version_id}</strong> · {action.predecessor.entries.length} 条</p><details><summary>新目录技术标识</summary><label>新目录版本 ID<input className="mono" required maxLength={128} value={id} onChange={event=>setId(event.target.value)}/></label></details><label>生效时间（本地时区）<input type="datetime-local" required value={effective} onChange={event=>setEffective(event.target.value)}/></label></fieldset></form>}
    {error?<p role="alert">{error}</p>:null}
  </Sheet>;
}
