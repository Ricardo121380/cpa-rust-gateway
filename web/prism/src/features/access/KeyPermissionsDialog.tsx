import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import { ConfigurationTaskNotice } from "../config-versions/ConfigurationTaskNotice";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import type { PublicModel, RoutingPage, RouteListItem } from "../models/model";
import { editedExpiry, toLocalInput, type AccessGroupRecord, type ClientKeyRecord } from "./model";

type Grant = Readonly<{route_id:string;enabled:boolean}>;
async function readRoutes(read:<T>(operation:"listRoutes",request:{query:{limit:number;cursor?:string}})=>Promise<T>) {
  const routes:RouteListItem[]=[];let cursor:string|undefined;let revision:string|undefined;
  do {
    const page=await read<RoutingPage<RouteListItem>>("listRoutes",{query:{limit:100,...(cursor?{cursor}:{})}});
    if(revision!==undefined&&revision!==page.revision)throw new Error("模型配置已变化，请重新读取。");
    revision=page.revision;routes.push(...page.items);cursor=page.next_cursor??undefined;
    if(cursor&&routes.length>=10000)throw new Error("模型连接超出本次编辑范围，请使用高级维护。");
  }while(cursor);
  return routes;
}
const fingerprint=(grants:readonly Grant[])=>JSON.stringify([...grants].sort((a,b)=>a.route_id.localeCompare(b.route_id)).map(g=>[g.route_id,g.enabled]));

export function KeyPermissionsDialog({record,onClose,onSaved}:Readonly<{record:ClientKeyRecord;onClose:()=>void;onSaved:(version:ConfigVersionSummary)=>void}>) {
  const context=useVersionStore(s=>s.context);const queryClient=useQueryClient();
  const [workingId,setWorkingId]=useState<string>();
  const [selected,setSelected]=useState<ReadonlySet<string>>();
  const [name,setName]=useState<string>();const [dirty,setDirty]=useState(false);
  const source=useQuery({queryKey:["key-permissions",context?.configVersionId,context?.revision,record.id],enabled:!!context,queryFn:async()=>{
    const [groups,models,grants,routes]=await Promise.all([
      call<AccessGroupRecord[]>("listAccessGroups",{},{versionScoped:true}),
      call<PublicModel[]>("listPublicModels",{},{versionScoped:true}),
      call<Grant[]>("listAccessGroupRoutes",{path:{access_group_id:record.access_group_id}},{versionScoped:true}),
      readRoutes((operation,request)=>call(operation,request,{versionScoped:true})),
    ]);
    const group=groups.find(g=>g.id===record.access_group_id);if(!group)throw new Error("密钥的权限配置不存在，请重新读取。");
    return {group,models,grants,routes};
  }});
  const original=new Set(source.data?.routes.filter(r=>source.data?.grants.some(g=>g.route_id===r.id&&g.enabled)).map(r=>r.public_model_id)??[]);
  const chosen=selected??original;
  const save=useMutation({mutationFn:async(input:{status:ClientKeyRecord["status"];expiry:string})=>{
    const baseline=source.data;if(!baseline)throw new Error("请先读取模型权限。");
    const label=(name??baseline.group.name).trim();if(!label)throw new Error("请输入名称。");
    const task=await beginConfigurationTask(`修改密钥 · ${label}`);setWorkingId(task.version.id);
    const keys=await task.read<ClientKeyRecord[]>("listClientKeys");
    const current=keys.find(k=>k.id===record.id);
    if(!current||current.status!==record.status||current.access_group_id!==record.access_group_id||(current.expires_at_ms??null)!==(record.expires_at_ms??null))throw new Error("密钥已被修改，请关闭后重新读取。");
    const groups=await task.read<AccessGroupRecord[]>("listAccessGroups");const group=groups.find(g=>g.id===record.access_group_id);
    const grants=await task.read<Grant[]>("listAccessGroupRoutes",{path:{access_group_id:record.access_group_id}});
    const routes=await readRoutes(task.read);
    if(!group||JSON.stringify(group)!==JSON.stringify(baseline.group)||fingerprint(grants)!==fingerprint(baseline.grants)||JSON.stringify(routes)!==JSON.stringify(baseline.routes))throw new Error("模型权限已变化，请关闭后重新读取。");
    if(Object.keys(group.limits).length>0&&keys.some(k=>k.id!==record.id&&k.access_group_id===group.id))throw new Error("此密钥与其他密钥共用额度限制，请在高级访问组中修改，避免拆分后改变总额度。");
    // A private replacement group prevents an edit from changing sibling keys. Keep limits and
    // group status, and retain exact old grants for retained models instead of broadening sources.
    const target=`group-${crypto.randomUUID()}`;
    const retained=grants.filter(g=>{const route=routes.find(r=>r.id===g.route_id);return route&&chosen.has(route.public_model_id)&&original.has(route.public_model_id);});
    const added=routes.filter(r=>chosen.has(r.public_model_id)&&!original.has(r.public_model_id));
    await task.mutate("createAccessGroup",{body:{id:target,name:label,status:group.status,limits:group.limits}});
    for(const grant of [...retained,...added.map(r=>({route_id:r.id,enabled:true}))])await task.mutate("grantAccessGroupRoute",{path:{access_group_id:target},body:grant});
    await task.mutate("updateClientKey",{path:{client_key_id:record.id},body:{id:record.id,access_group_id:target,status:input.status,expires_at_ms:editedExpiry(input.expiry,record.expires_at_ms)}});
    return task.finish();
  },onSuccess:version=>{void queryClient.invalidateQueries({queryKey:["client-keys"]});void queryClient.invalidateQueries({queryKey:["access-groups"]});onSaved(version);}});
  const close=()=>{if(!save.isPending)onClose();};
  const submit=(event:FormEvent<HTMLFormElement>)=>{event.preventDefault();const data=new FormData(event.currentTarget);save.mutate({status:String(data.get("status")) as ClientKeyRecord["status"],expiry:String(data.get("expiry")??"")});};
  return <Sheet title="编辑 API 密钥" onEscape={close}>
    {source.isError?<p role="alert">{asAppError(source.error).message}<button onClick={()=>void source.refetch()}>重新读取</button></p>:null}
    {source.isPending?<p role="status">正在读取模型权限…</p>:null}
    {source.data?<form className="sheet-form" onSubmit={submit} onChange={()=>setDirty(true)}>
      <fieldset disabled={save.isPending||workingId!==undefined}>
        <label>名称<input required maxLength={128} value={name??source.data.group.name} onChange={e=>setName(e.target.value)}/></label>
        <label>状态<select name="status" defaultValue={record.status}><option value="active" disabled={record.status==="revoked"}>启用</option><option value="disabled" disabled={record.status==="revoked"}>停用</option><option value="revoked">吊销</option></select></label>
        <label>有效期<input name="expiry" type="datetime-local" defaultValue={toLocalInput(record.expires_at_ms)}/></label>
        <p className="muted">有效期留空表示不过期。</p>
        <fieldset><legend>允许模型 · {chosen.size}</legend>
          <div className="data-toolbar"><button type="button" className="secondary" onClick={()=>{setSelected(new Set(source.data?.models.filter(m=>m.status==="active").map(m=>m.id)));setDirty(true);}}>全选当前已开放模型</button><button type="button" className="secondary" onClick={()=>{setSelected(new Set());setDirty(true);}}>清空选择</button></div>
          {source.data.models.filter(m=>m.status==="active"||original.has(m.id)).map(m=><label className="check-row" key={m.id}><input type="checkbox" checked={chosen.has(m.id)} onChange={()=>{const next=new Set(chosen);if(next.has(m.id))next.delete(m.id);else next.add(m.id);setSelected(next);}}/>{m.model_name}{m.status!=="active"?"（已关闭）":""}</label>)}
        </fieldset>
        {chosen.size===0?<p role="status">此密钥将无法调用任何模型。</p>:null}
      </fieldset>
      <div className="sheet-actions"><button type="button" className="secondary" disabled={save.isPending} onClick={close}>取消</button><button disabled={!dirty||save.isPending||workingId!==undefined}>保存并应用</button></div>
    </form>:null}
    <ConfigurationTaskNotice workingId={workingId} error={save.error} onReview={onSaved}/>
  </Sheet>;
}
