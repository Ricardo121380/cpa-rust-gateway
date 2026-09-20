import {useEffect,useRef,useState,type FormEvent} from "react";
import {isCancelledError,useMutation} from "@tanstack/react-query";
import {Sheet,SheetDismissButton} from "../../components/Sheet";
import {ResourceIdInput} from "../../components/ResourceIdentity";
import {asAppError} from "../../api/errors";
import {beginConfigurationTask} from "../config-versions/configurationTask";
import {ConfigurationTaskNotice} from "../config-versions/ConfigurationTaskNotice";
import {useVersionStore,type ConfigVersionSummary} from "../config-versions/versionStore";
import {useSessionStore} from "../../session/sessionStore";
import {formatLimits,type AccessGroupRecord} from "./model";

export function GroupMaintenanceDialog({record,removing=false,onClose,onSelected}:Readonly<{record:AccessGroupRecord|null;removing?:boolean;onClose:()=>void;onSelected:(version:ConfigVersionSummary)=>void}>){
 const [owner]=useState(()=>({session:useSessionStore.getState().generation,selection:useVersionStore.getState().selectionGeneration,context:useVersionStore.getState().context}));
 const live=useRef(true),submitted=useRef(false);useEffect(()=>{live.current=true;return()=>{live.current=false;};},[]);
 const owned=()=>live.current&&owner.session===useSessionStore.getState().generation&&owner.selection===useVersionStore.getState().selectionGeneration;
 const [error,setError]=useState<string>(),[workingId,setWorkingId]=useState<string>(),[receipt,setReceipt]=useState<ConfigVersionSummary>();
 const save=useMutation({mutationFn:async(input?:AccessGroupRecord)=>{
  if(!owner.context||!owned())throw new Error("配置上下文已失效，请重新打开。");
  const task=await beginConfigurationTask(removing?"删除访问组":"维护访问组",{id:owner.context.configVersionId,revision:owner.context.revision},"deferred");
  if(!owned())return;setWorkingId(task.version.id);useVersionStore.getState().rememberPending(task.version);
  if(record){const current=(await task.read<AccessGroupRecord[]>("listAccessGroups")).find(group=>group.id===record.id);
   if(!current||current.name!==record.name||current.status!==record.status||JSON.stringify(Object.entries(current.limits).sort())!==JSON.stringify(Object.entries(record.limits).sort()))throw new Error("访问组已变化，请重新读取后操作。");}
  await task.mutate(removing?"deleteAccessGroup":record?"updateAccessGroup":"createAccessGroup",{...(record?{path:{access_group_id:record.id}}:{}),...(removing?{}:{body:input})});
  return task.finish();
 },onSuccess:version=>{if(version&&owned()){useVersionStore.getState().rememberPending(version);if(useVersionStore.getState().context?.configVersionId===version.id)useVersionStore.getState().advanceFromEtag(version.revision);setReceipt(version);}},onError:cause=>{if(owned()&&!isCancelledError(cause))setError(asAppError(cause).message);}});
 const submit=(event:FormEvent<HTMLFormElement>)=>{event.preventDefault();if(submitted.current)return;const data=new FormData(event.currentTarget);
  const limits=data.get("clear_limits")==="on"?{}:record?.limits??{};const status=data.get("status")==="disabled"?"disabled":"active";
  if(status==="active"&&Object.keys(limits).length){setError("当前网关不支持访问组限额。请明确清除历史限制，或将访问组停用后保存。");return;}
  setError(undefined);submitted.current=true;save.mutate({id:record?.id??String(data.get("id")??"").trim(),name:String(data.get("name")??"").trim(),status,limits});
 };
 const review=(version:ConfigVersionSummary)=>{if(owned()){useVersionStore.getState().rememberPending(version);onSelected(version);}};
 const title=receipt?"访问组修改结果":removing?"确认删除访问组":record?`编辑访问组 · ${record.name}`:"新建访问组";
 return <Sheet title={title} layout={removing?"confirm":"form"} busy={save.isPending} guardUnsaved={!receipt&&!removing} onEscape={onClose} footer={receipt?<><SheetDismissButton className="secondary" onDismiss={onClose}>关闭</SheetDismissButton><button onClick={()=>review(receipt)}>查看工作草稿</button></>:<><SheetDismissButton className="secondary" disabled={save.isPending} onDismiss={onClose}>取消</SheetDismissButton>{removing?<button className="danger" disabled={submitted.current} onClick={()=>{if(!submitted.current){submitted.current=true;save.mutate(undefined);}}}>确认删除</button>:<button type="submit" form="group-maintenance-form" disabled={submitted.current}>{record?"保存":"创建"}</button>}</>}>
  {receipt?<p role="status">修改已保存到草稿，尚未应用。请在待应用变更中统一核对并应用。</p>:removing?<p>删除 {record?.name} 及其路由授权。关联密钥的后续访问会受影响；历史请求与账本保留。</p>:<form id="group-maintenance-form" className="sheet-form" onSubmit={submit}><fieldset disabled={submitted.current}>
   <label>{record?"访问组":"访问组标识"}<ResourceIdInput kind="group" name="id" required maxLength={128} readOnly={!!record} defaultValue={record?.id??""}/></label>
   <label>名称<input name="name" required maxLength={128} defaultValue={record?.name??""}/></label>
   <label>状态<select name="status" defaultValue={record?.status??"active"}><option value="active">active</option><option value="disabled">disabled</option></select></label>
   {record&&Object.keys(record.limits).length?<div><p>历史限制 <code>{formatLimits(record.limits)}</code></p><p>当前网关不支持执行这些限制。保留限制时仅可保存为停用状态。</p><label className="check-row"><input type="checkbox" name="clear_limits"/>清除历史限制</label></div>:<p className="stat-sub">当前网关不支持访问组限额，此访问组不设置限额。</p>}
  </fieldset></form>}
  {!receipt&&error?<p role="alert">{error}</p>:null}
  {!receipt&&save.isError?<><p>本次操作未完整确认，不会重复提交；已保存的部分请读取工作草稿核对。</p><ConfigurationTaskNotice workingId={workingId} error={save.error} onReview={review}/></>:null}
 </Sheet>;
}
