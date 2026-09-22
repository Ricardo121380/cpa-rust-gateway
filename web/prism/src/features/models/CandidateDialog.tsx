import { CancelledError, isCancelledError, useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { asAppError } from "../../api/errors";
import { ResourcePicker } from "../../components/ResourcePicker";
import { IdentityDetails } from "../../components/ResourceIdentity";
import { InlineWorkspace } from "../../components/InlineWorkspace";
import { useOperationBoundary } from "../../components/OperationBoundary";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore } from "../config-versions/versionStore";
import { sameRoute } from "./advancedRoutingModel";
import { isModelActionOwner, readRoutingInventory, runDraftRoutingWrite, type DraftRoutingOwner } from "./advancedRoutingTask";
import { sameCandidate, type ModelTaskReceipt } from "./modelTask";
import { CREDENTIAL_SCOPE, capabilityOverrideNeedsJson, formatCapabilityOverride, parseCapabilityOverride, parseCapabilityOverrideJson, TRANSFORM_MODES, transformModeHint, validCandidateParams, type CandidateRecord, type RouteListItem, type TransformMode } from "./model";

type CandidateInput=Readonly<{id:string;endpoint_id:string;upstream_model:string;credential_scope:typeof CREDENTIAL_SCOPE;transform_mode:TransformMode;enabled:boolean;priority:number;weight:number;capability_override:Readonly<Record<string,boolean>>}>;
type Form={id:string;endpoint:string;model:string;mode:TransformMode;enabled:boolean;priority:string;weight:string;overrides:string};
export type CandidateAction=Readonly<{kind:"add"|"edit"|"delete";owner:DraftRoutingOwner;route:RouteListItem;candidate?:CandidateRecord;seed?:{model:string;endpoint:string}}>;

export function CandidateDialog({action,onClose,onDone}:Readonly<{action:CandidateAction;onClose:()=>void;onDone:(receipt:ModelTaskReceipt,routeId:string)=>void}>){
  const boundary=useOperationBoundary();
  const initial=action.candidate;
  const originalOverride=initial?.capability_override??{};
  const jsonOverride=capabilityOverrideNeedsJson(originalOverride);
  const originalOverrideText=jsonOverride?JSON.stringify(originalOverride,null,2):formatCapabilityOverride(originalOverride);
  const [form,setForm]=useState<Form>({id:initial?.id??"",endpoint:initial?.endpoint_id??action.seed?.endpoint??"",model:initial?.upstream_model??action.seed?.model??"",mode:initial?.transform_mode??"passthrough",enabled:initial?.enabled??true,priority:String(initial?.priority??0),weight:String(initial?.weight??1),overrides:originalOverrideText});
  const [receipt,setReceipt]=useState<ModelTaskReceipt>();
  const [error,setError]=useState<string>();
  const submitted=useRef(false);
  const formId="candidate-action-form";
  useEffect(()=>{
    const closeIfLost=()=>{if(!isModelActionOwner(action.owner))onClose();};
    const stopVersion=useVersionStore.subscribe(closeIfLost);
    const stopSession=useSessionStore.subscribe(closeIfLost);
    return ()=>{stopVersion();stopSession();};
  },[action.owner,onClose]);
  const write=useMutation({mutationFn:async(body:CandidateInput|undefined)=>{
    if(!isModelActionOwner(action.owner))throw new CancelledError({silent:true});
    const operation=action.kind==="add"?"createRouteCandidate":action.kind==="edit"?"updateRouteCandidate":"deleteRouteCandidate";
    const outcome=await runDraftRoutingWrite<CandidateRecord|undefined>(action.owner,operation,action.kind==="add"?{path:{route_id:action.route.id},body}:{path:{route_id:action.route.id,candidate_id:initial?.id??""},...(body?{body}:{})},async(read)=>{
      const routes=await readRoutingInventory<RouteListItem>(read,"listRoutes",action.owner);
      if(!sameRoute(routes.find(row=>row.id===action.route.id),action.route))throw new Error("所属路由已变化，请重新读取后操作。");
      const candidates=await readRoutingInventory<CandidateRecord>(read,"listRouteCandidates",action.owner);
      if(action.kind==="add"){
        if(candidates.some(row=>row.id===body?.id))throw new Error("候选标识已存在，请重新核对。");
      }else if(!initial||!sameCandidate(candidates.find(row=>row.id===initial.id&&row.route_id===action.route.id),initial))throw new Error("候选已变化，请重新读取后操作。");
      if(body){
        const endpoints=await readRoutingInventory<{id:string}>(read,"listManagedEndpoints",action.owner);
        if(!endpoints.some(endpoint=>endpoint.id===body.endpoint_id))throw new Error("接口连接已不存在，请重新选择。");
      }
    });
    return outcome.receipt;
  },onSuccess:next=>{if(isModelActionOwner(action.owner))setReceipt(next);},onError:cause=>{if(!isCancelledError(cause)&&isModelActionOwner(action.owner)){submitted.current=false;setError(asAppError(cause).message);}}});
  const submit=(event:FormEvent)=>{
    event.preventDefault();
    if(submitted.current||!isModelActionOwner(action.owner))return;
    if(action.kind==="delete"){submitted.current=true;write.mutate(undefined);return;}
    const priority=Number(form.priority),weight=Number(form.weight);
    if(!validCandidateParams(priority,weight)){setError("优先级须为非负整数，权重须为 1–10000。");return;}
    const parsed=form.overrides===originalOverrideText
      ? {ok:true as const,override:{...originalOverride}}
      : jsonOverride?parseCapabilityOverrideJson(form.overrides):parseCapabilityOverride(form.overrides);
    if(!parsed.ok){setError(parsed.reason);return;}
    if(!form.id.trim()||!form.endpoint||!form.model.trim()){setError("请填写候选标识、接口连接和上游原始模型 ID。");return;}
    submitted.current=true;setError(undefined);
    write.mutate({id:form.id.trim(),endpoint_id:form.endpoint,upstream_model:form.model.trim(),credential_scope:CREDENTIAL_SCOPE,transform_mode:form.mode,enabled:form.enabled,priority,weight,capability_override:parsed.override});
  };
  const done=()=>{if(receipt)onDone(receipt,action.route.id);else onClose();};
  const title=receipt?"候选配置结果":action.kind==="add"?"添加模型来源":action.kind==="edit"?"编辑模型来源":"删除模型来源";
  const inline=action.kind!=="delete";
  const dirty=!receipt&&(form.id!==(initial?.id??"")||form.endpoint!==(initial?.endpoint_id??action.seed?.endpoint??"")||form.model!==(initial?.upstream_model??action.seed?.model??"")||form.mode!==(initial?.transform_mode??"passthrough")||form.enabled!==(initial?.enabled??true)||form.priority!==String(initial?.priority??0)||form.weight!==String(initial?.weight??1)||form.overrides!==originalOverrideText);
  const footer=receipt?<button type="button" onClick={done}>{receipt.kind==="unconfirmed"?"核对草稿":"完成"}</button>:<>{inline?<button type="button" className="secondary" disabled={write.isPending} onClick={()=>boundary.request(()=>{})}>取消</button>:<SheetDismissButton className="secondary" disabled={write.isPending}>取消</SheetDismissButton>}<button type={action.kind==="delete"?"button":"submit"} form={action.kind==="delete"?undefined:formId} className={action.kind==="delete"?"danger":undefined} disabled={write.isPending||submitted.current} onClick={action.kind==="delete"?()=>{if(!submitted.current){submitted.current=true;write.mutate(undefined);}}:undefined}>{action.kind==="delete"?"确认删除":action.kind==="add"?"创建候选":"保存候选"}</button></>;
  const content=<>
    {receipt?<p role={receipt.kind==="unconfirmed"?"alert":"status"}>{receipt.message}</p>:action.kind==="delete"?<p className="reveal-warning">移除 <strong className="mono">{initial?.upstream_model}</strong> 这条来源。若它是最后一条启用的候选，草稿拓扑校验会失败；不会自动停用公开模型。</p>:<form id={formId} className="sheet-form" onSubmit={submit}><fieldset disabled={write.isPending||submitted.current}>
      {initial?null:<label>候选标识<input className="mono" required maxLength={128} value={form.id} onChange={event=>setForm({...form,id:event.target.value})}/></label>}
      <label>接口连接<ResourcePicker kind="endpoint" required value={form.endpoint} onChange={value=>setForm({...form,endpoint:value})}/></label>
      <label>上游原始模型 ID<input className="mono" required maxLength={256} value={form.model} onChange={event=>setForm({...form,model:event.target.value})}/></label>
      <label>协议转换<select value={form.mode} onChange={event=>setForm({...form,mode:event.target.value as TransformMode})}>{TRANSFORM_MODES.map(mode=><option key={mode} value={mode}>{transformModeHint(mode)}</option>)}</select></label>
      <label className="toggle-row"><input type="checkbox" checked={form.enabled} onChange={event=>setForm({...form,enabled:event.target.checked})}/>启用此来源</label>
      <div className="resource-editor-grid">
        <label>优先级<input type="number" min={0} value={form.priority} onChange={event=>setForm({...form,priority:event.target.value})}/></label>
        <label>权重<input type="number" min={1} max={10000} value={form.weight} onChange={event=>setForm({...form,weight:event.target.value})}/></label>
      </div>
      <label>能力覆盖（可留空）{jsonOverride?<><textarea className="mono" rows={5} maxLength={8192} value={form.overrides} onChange={event=>setForm({...form,overrides:event.target.value})}/><small>此候选包含特殊能力键；使用 JSON 对象编辑，保持键名原样。</small></>:<input className="mono" maxLength={512} placeholder="vision=true tools=false" value={form.overrides} onChange={event=>setForm({...form,overrides:event.target.value})}/>}</label>
    </fieldset></form>}
    {!receipt&&initial&&inline?<IdentityDetails entries={[["候选",initial.id,initial.upstream_model]]}/>:null}
    {error?<p role="alert">{error}</p>:null}
  </>;
  return inline?<InlineWorkspace title={title} description="使用实际接口与上游原始模型 ID；调度参数只影响这条来源。" busy={write.isPending} dirty={dirty} onClose={done} footer={footer}>{content}</InlineWorkspace>:<Sheet title={title} layout="confirm" tone={receipt?"default":"danger"} onEscape={done} busy={write.isPending} guardUnsaved={false} footer={footer}>{content}</Sheet>;
}
