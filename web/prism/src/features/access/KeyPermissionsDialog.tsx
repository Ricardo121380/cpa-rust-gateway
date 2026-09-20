import { useMutation, useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import type { PublicModel, RoutingPage, RouteListItem } from "../models/model";
import { runModelTask, type ModelTaskReceipt } from "../models/modelTask";
import { editedExpiry, toLocalInput, type AccessGroupRecord, type ClientKeyRecord } from "./model";

type Grant=Readonly<{route_id:string;enabled:boolean}>;
type Baseline=Readonly<{source:Readonly<{id:string;revision:string}>;key:ClientKeyRecord;group:AccessGroupRecord;models:readonly PublicModel[];grants:readonly Grant[];routes:readonly RouteListItem[]}>;

async function readRoutes(read:<T>(operation:"listRoutes",request:{query:{limit:number;cursor?:string}})=>Promise<T>){
  const routes:RouteListItem[]=[];let cursor:string|undefined;let revision:string|undefined;
  do{
    const page=await read<RoutingPage<RouteListItem>>("listRoutes",{query:{limit:100,...(cursor?{cursor}:{})}});
    if(revision!==undefined&&revision!==page.revision)throw new Error("模型配置已变化，请重新读取。");
    revision=page.revision;routes.push(...page.items);cursor=page.next_cursor??undefined;
    if(cursor&&routes.length>=10000)throw new Error("模型连接超出本次编辑范围，请使用高级维护。");
  }while(cursor);
  return routes;
}

const sameKey=(a:ClientKeyRecord,b:ClientKeyRecord)=>a.id===b.id&&a.access_group_id===b.access_group_id&&a.status===b.status&&(a.expires_at_ms??null)===(b.expires_at_ms??null);
const sameLimits=(a:Readonly<Record<string,number>>,b:Readonly<Record<string,number>>)=>Object.keys(a).length===Object.keys(b).length&&Object.keys(a).every(key=>a[key]===b[key]);
const sameGroup=(a:AccessGroupRecord,b:AccessGroupRecord)=>a.id===b.id&&a.name===b.name&&a.status===b.status&&sameLimits(a.limits,b.limits);
const sameGrants=(a:readonly Grant[],b:readonly Grant[])=>a.length===b.length&&a.every(row=>b.some(other=>other.route_id===row.route_id&&other.enabled===row.enabled));
const sameRoutes=(a:readonly RouteListItem[],b:readonly RouteListItem[])=>a.length===b.length&&a.every(row=>b.some(other=>other.id===row.id&&other.public_model_id===row.public_model_id&&other.policy===row.policy&&other.max_attempts===row.max_attempts&&other.bootstrap_timeout_ms===row.bootstrap_timeout_ms));
const allowedModels=(routes:readonly RouteListItem[],grants:readonly Grant[])=>new Set(routes.filter(route=>grants.some(grant=>grant.route_id===route.id&&grant.enabled)).map(route=>route.public_model_id));
const sameSelection=(a:ReadonlySet<string>,b:ReadonlySet<string>)=>a.size===b.size&&[...a].every(id=>b.has(id));

export function KeyPermissionsDialog({record,onClose,onSaved}:Readonly<{record:ClientKeyRecord;onClose:()=>void;onSaved:(version:ConfigVersionSummary)=>void}>){
  const formId="key-permissions-form";
  const context=useVersionStore(state=>state.context);
  const [baseline,setBaseline]=useState<Baseline>();
  const [selected,setSelected]=useState<ReadonlySet<string>>();
  const [name,setName]=useState("");
  const [status,setStatus]=useState<ClientKeyRecord["status"]>(record.status);
  const [expiry,setExpiry]=useState(toLocalInput(record.expires_at_ms));
  const [receipt,setReceipt]=useState<ModelTaskReceipt>();
  const submitted=useRef(false);
  const source=useQuery({queryKey:["key-permissions",context?.configVersionId,context?.revision,record.id],enabled:!!context,retry:false,refetchOnMount:"always",queryFn:async():Promise<Baseline>=>{
    if(!context)throw new Error("请选择配置后再编辑密钥。");
    const before=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:context.configVersionId}});
    if(before.revision!==context.revision)throw new Error("配置已变化，请重新读取。");
    const [keys,groups,models,grants,routes]=await Promise.all([
      call<ClientKeyRecord[]>("listClientKeys",{},{versionScoped:true}),
      call<AccessGroupRecord[]>("listAccessGroups",{},{versionScoped:true}),
      call<PublicModel[]>("listPublicModels",{},{versionScoped:true}),
      call<Grant[]>("listAccessGroupRoutes",{path:{access_group_id:record.access_group_id}},{versionScoped:true}),
      readRoutes((operation,request)=>call(operation,request,{versionScoped:true})),
    ]);
    const after=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:context.configVersionId}});
    if(after.revision!==before.revision)throw new Error("读取过程中配置已变化，请重新读取。");
    const key=keys.find(item=>item.id===record.id);
    const group=groups.find(item=>item.id===record.access_group_id);
    if(!key||!sameKey(key,record)||!group)throw new Error("密钥或权限配置已变化，请重新读取列表。");
    return {source:{id:context.configVersionId,revision:context.revision},key,group,models,grants,routes};
  }});
  useEffect(()=>{
    if(baseline||!source.isSuccess||source.isFetching)return;
    setBaseline(source.data);
    setSelected(allowedModels(source.data.routes,source.data.grants));
    setName(source.data.group.name);
    setStatus(source.data.key.status);
    setExpiry(toLocalInput(source.data.key.expires_at_ms));
  },[baseline,source.data,source.isSuccess,source.isFetching]);
  const original=baseline?allowedModels(baseline.routes,baseline.grants):new Set<string>();
  const chosen=selected??original;
  const dirty=!!baseline&&(!sameSelection(chosen,original)||name!==baseline.group.name||status!==baseline.key.status||expiry!==toLocalInput(baseline.key.expires_at_ms));
  const save=useMutation({mutationFn:async(input:Readonly<{baseline:Baseline;chosen:ReadonlySet<string>;label:string;status:ClientKeyRecord["status"];expiry:string}>)=>runModelTask(`修改密钥 · ${input.label}`,async task=>{
    const keys=await task.read<ClientKeyRecord[]>("listClientKeys");
    const current=keys.find(key=>key.id===input.baseline.key.id);
    if(!current||!sameKey(current,input.baseline.key))throw new Error("密钥已被修改，请关闭后重新读取。");
    const groups=await task.read<AccessGroupRecord[]>("listAccessGroups");
    const group=groups.find(item=>item.id===input.baseline.group.id);
    const grants=await task.read<Grant[]>("listAccessGroupRoutes",{path:{access_group_id:input.baseline.group.id}});
    const routes=await readRoutes(task.read);
    const models=await task.read<PublicModel[]>("listPublicModels");
    if(!group||!sameGroup(group,input.baseline.group)||!sameGrants(grants,input.baseline.grants)||!sameRoutes(routes,input.baseline.routes))throw new Error("模型权限已变化，请关闭后重新读取。");
    if(Object.keys(group.limits).length>0&&keys.some(key=>key.id!==current.id&&key.access_group_id===group.id))throw new Error("此密钥与其他密钥共用额度限制，请在高级访问组中修改。");
    for(const id of input.chosen){
      if(!routes.some(route=>route.public_model_id===id))throw new Error("所选模型缺少可用路由，请先连接模型。");
      if(!original.has(id)&&!models.some(model=>model.id===id&&model.status==="active"))throw new Error("所选模型已关闭或变化，请重新选择。");
    }
    const target=`group-${crypto.randomUUID()}`;
    const retained=grants.filter(grant=>{const route=routes.find(row=>row.id===grant.route_id);return route&&input.chosen.has(route.public_model_id)&&original.has(route.public_model_id);}).map(grant=>({route_id:grant.route_id,enabled:grant.enabled}));
    const added=routes.filter(route=>input.chosen.has(route.public_model_id)&&!original.has(route.public_model_id));
    await task.mutate("createAccessGroup",{body:{id:target,name:input.label,status:group.status,limits:group.limits}});
    for(const grant of [...retained,...added.map(route=>({route_id:route.id,enabled:true}))])await task.mutate("grantAccessGroupRoute",{path:{access_group_id:target},body:grant});
    await task.mutate("updateClientKey",{path:{client_key_id:current.id},body:{id:current.id,access_group_id:target,status:input.status,expires_at_ms:editedExpiry(input.expiry,current.expires_at_ms)}});
  },{expectedSource:input.baseline.source,probeUnchanged:true}),onSuccess:({receipt:next})=>setReceipt(next),onError:()=>{submitted.current=false;}});
  const done=()=>{if(receipt&&receipt.kind!=="unchanged")onSaved(receipt.workingVersion);else onClose();};
  const submit=(event:FormEvent<HTMLFormElement>)=>{
    event.preventDefault();
    if(!baseline||!selected||submitted.current||!dirty)return;
    const label=name.trim();if(!label)return;
    submitted.current=true;
    save.mutate({baseline,chosen:new Set(selected),label,status,expiry});
  };
  return <Sheet title={receipt?"密钥权限结果":"编辑 API 密钥"} description={receipt?"请核对保存与应用状态。":"修改名称、状态、有效期和允许调用的模型。"} onEscape={()=>!save.isPending&&done()} busy={save.isPending} isDirty={!receipt&&dirty} footer={receipt?<SheetDismissButton onDismiss={done}>{receipt.kind==="saved_applied"||receipt.kind==="unchanged"?"完成":"核对配置"}</SheetDismissButton>:baseline?<><SheetDismissButton className="secondary" disabled={save.isPending}>取消</SheetDismissButton><button type="submit" form={formId} disabled={!dirty||save.isPending||submitted.current}>{context?.status==="draft"?"保存到草稿":"保存并应用"}</button></>:undefined}>
    {receipt?<div role="status"><p>{receipt.message}</p><p>已确认保存 {receipt.acknowledgedWrites} 步。</p></div>:<>
      {source.isError?<p role="alert">{asAppError(source.error).message}<button type="button" onClick={()=>void source.refetch()}>重新读取</button></p>:null}
      {source.isPending||source.isFetching&&!baseline?<p role="status">正在读取模型权限…</p>:null}
      {baseline?<form id={formId} className="sheet-form" onSubmit={submit}>
        <fieldset disabled={save.isPending||submitted.current}>
          <label>名称<input required maxLength={128} value={name} onChange={event=>setName(event.target.value)}/></label>
          <label>状态<select value={status} onChange={event=>setStatus(event.target.value as ClientKeyRecord["status"])}><option value="active" disabled={baseline.key.status==="revoked"}>启用</option><option value="disabled" disabled={baseline.key.status==="revoked"}>停用</option><option value="revoked">吊销</option></select></label>
          <label>有效期<input type="datetime-local" value={expiry} onChange={event=>setExpiry(event.target.value)}/></label>
          <p className="muted">有效期留空表示不过期。</p>
          <fieldset><legend>允许模型 · {chosen.size}</legend>
            <div className="data-toolbar"><button type="button" className="secondary" onClick={()=>setSelected(new Set(baseline.models.filter(model=>model.status==="active").map(model=>model.id)))}>全选当前已开放模型</button><button type="button" className="secondary" onClick={()=>setSelected(new Set())}>清空选择</button></div>
            {baseline.models.filter(model=>model.status==="active"||original.has(model.id)).map(model=><label className="check-row" key={model.id}><input type="checkbox" checked={chosen.has(model.id)} onChange={()=>{const next=new Set(chosen);if(next.has(model.id))next.delete(model.id);else next.add(model.id);setSelected(next);}}/>{model.model_name}{model.status!=="active"?"（已关闭）":""}</label>)}
          </fieldset>
          {chosen.size===0?<p role="status">此密钥将无法调用任何模型。</p>:null}
        </fieldset>
      </form>:null}
      {save.isError?<p role="alert">{asAppError(save.error).message}</p>:null}
    </>}
  </Sheet>;
}
