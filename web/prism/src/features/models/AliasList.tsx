import {useMutation,useQuery,useIsMutating} from "@tanstack/react-query";
import {useState} from "react";
import {call} from "../../api/client";
import {asAppError} from "../../api/errors";
import {useVersionStore,type ConfigVersionSummary} from "../config-versions/versionStore";
import {beginConfigurationTask} from "../config-versions/configurationTask";
import {ConfigurationTaskNotice} from "../config-versions/ConfigurationTaskNotice";
import type {AliasRecord,PublicModel,RoutingPage} from "./model";

export function AliasList({model,onRemoved}:Readonly<{model:PublicModel;onRemoved:(version:ConfigVersionSummary)=>void}>) {
 const busy=useIsMutating({mutationKey:["model-alias-write"]})>0;
 const context=useVersionStore(s=>s.context);const [selected,setSelected]=useState<string>();const [workingId,setWorkingId]=useState<string>();
 const aliases=useQuery({queryKey:["model-aliases",context?.configVersionId,context?.revision,model.id],enabled:!!context,retry:false,queryFn:async()=>{
  const items:AliasRecord[]=[];let cursor:string|undefined;let revision:string|undefined;
  do {const page=await call<RoutingPage<AliasRecord>>("listModelAliases",{query:{limit:100,...(cursor?{cursor}:{})}},{versionScoped:true});if(revision&&revision!==page.revision)throw new Error("别名已改变，请重新读取。");revision=page.revision;items.push(...page.items);cursor=page.next_cursor??undefined;if(cursor&&items.length>=10000)throw new Error("别名超过本次读取范围。");}while(cursor);
  return items.filter(row=>row.public_model_id===model.id);
 }});
 const remove=useMutation({mutationKey:["model-alias-write"],mutationFn:async(alias:string)=>{const task=await beginConfigurationTask("删除模型别名");setWorkingId(task.version.id);await task.mutate("deleteModelAlias",{path:{public_model_id:model.id},body:{alias}});return task.finish();},onSuccess:onRemoved});
 return <section className="alias-list" aria-label="已有别名"><h3>已有别名</h3>
  {aliases.isPending?<p role="status">正在读取…</p>:aliases.isError?<p role="alert">{asAppError(aliases.error).message}<button type="button" onClick={()=>void aliases.refetch()}>重新读取</button></p>:!aliases.data?.length?<p className="muted">没有别名，客户端直接使用 {model.model_name}。</p>:aliases.data.map(row=><div className="data-toolbar" key={row.alias}><code>{row.alias}</code><button type="button" className="secondary" disabled={busy||workingId!==undefined} onClick={()=>setSelected(row.alias)}>删除</button></div>)}
  {selected?<div role="alert"><p>删除 {selected} 后，该名称不再可用。客户端应使用 {model.model_name}；模型和来源连接会保留。</p><div className="sheet-actions"><button type="button" className="secondary" disabled={remove.isPending} onClick={()=>setSelected(undefined)}>取消</button><button type="button" className="danger" disabled={busy||workingId!==undefined} onClick={()=>remove.mutate(selected)}>确认删除</button></div></div>:null}
  <ConfigurationTaskNotice workingId={workingId} error={remove.error} onReview={onRemoved}/>
 </section>;
}
