import { isCancelledError, useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { call } from "../api/client";
import { asAppError } from "../api/errors";
import { ResourceIdentity } from "../components/ResourceIdentity";
import { useOperationBoundary } from "../components/OperationBoundary";
import { Sheet, SheetDismissButton } from "../components/Sheet";
import { GlassSurface } from "../components/glass/GlassSurface";
import { LifecycleConfirmation } from "../features/config-versions/LifecycleConfirmation";
import { useVersionStore, type ConfigVersionSummary } from "../features/config-versions/versionStore";
import { useSessionStore } from "../session/sessionStore";

type Validation=Readonly<{valid:boolean;error_codes?:readonly string[]}>;
type Publication=Readonly<{active_config_version_id:string;replaced_config_version_id?:string|null}>;
type Source=Readonly<{id:string;revision:string;selection:number;session:number}>;
type Panel=
  |Readonly<{kind:"confirm";source:Source}>
  |Readonly<{kind:"validating";source:Source}>
  |Readonly<{kind:"validation";source:Source;result:Validation}>
  |Readonly<{kind:"published";source:Source;publication:Publication;summary?:ConfigVersionSummary;readError?:string}>
  |Readonly<{kind:"unconfirmed";source:Source;message:string}>;

const owned=(source:Source)=>{
  const state=useVersionStore.getState();
  return state.selectionGeneration===source.selection&&state.context?.configVersionId===source.id&&useSessionStore.getState().generation===source.session;
};

/** Draft dock is the third glass pane; its modal state is one transaction at a time. */
export function DraftDock(){
  const admission=useOperationBoundary();
  const queryClient=useQueryClient();
  const navigate=useNavigate();
  const context=useVersionStore(state=>state.context);
  const [panel,setPanel]=useState<Panel>();
  const [error,setError]=useState<string>();
  const source=():Source|undefined=>{
    const state=useVersionStore.getState();
    if(state.context?.status!=="draft")return undefined;
    return {id:state.context.configVersionId,revision:state.context.revision,selection:state.selectionGeneration,session:useSessionStore.getState().generation};
  };
  const validate=useMutation({mutationFn:async(target:Source)=>{
    if(!owned(target)||useVersionStore.getState().context?.revision!==target.revision)throw new Error("草稿已变化，请重新核对。");
    const before=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:target.id}});
    if(before.revision!==target.revision)throw new Error("草稿已变化，请重新核对。");
    const result=await call<Validation>("validateConfigVersion",{path:{config_version_id:target.id}});
    const after=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:target.id}});
    if(after.revision!==target.revision)throw new Error("校验期间草稿已变化，请重新校验。");
    return result;
  },onSuccess:(result,target)=>{setPanel(current=>current?.kind==="validating"&&current.source===target?(owned(target)?{kind:"validation",source:target,result}:undefined):current);},onError:(cause,target)=>{
    setPanel(current=>current?.kind==="validating"&&current.source===target?undefined:current);
    if(owned(target)&&!isCancelledError(cause))setError(asAppError(cause).message);
  }});
  const rereadPublished=async(publication:Publication,target:Source)=>{
    try{
      const versions=await call<ConfigVersionSummary[]>("listConfigVersions");
      if(!owned(target))return;
      const summary=versions.find(version=>version.id===publication.active_config_version_id&&version.status==="active");
      if(!summary)throw new Error("发布回执已收到，但活动版本尚未在列表中读到。");
      setPanel(current=>current?.kind==="published"&&current.source===target?{...current,summary,readError:undefined}:current);
    }catch(cause){
      if(!owned(target))return;
      setPanel(current=>current?.kind==="published"&&current.source===target?{...current,readError:asAppError(cause).message}:current);
    }
  };
  const publish=useMutation({mutationFn:async(input:Readonly<{source:Source;revision:string;expectedActive:string;lifecycleEvent:string}>):Promise<Readonly<{kind:"acknowledged";publication:Publication}|{kind:"unconfirmed";message:string}>>=>{
    const target=input.source;
    if(!owned(target)||target.revision!==input.revision||useVersionStore.getState().context?.revision!==target.revision)throw new Error("发布目标已变化，请重新核对。");
    try{
      const publication=await call<Publication>("publishConfigVersion",{path:{config_version_id:target.id},headers:{"If-Match":target.revision,"X-Expected-Active-Version":input.expectedActive,"X-Expected-Lifecycle-Event":input.lifecycleEvent}});
      if(publication.active_config_version_id!==target.id)return {kind:"unconfirmed",message:"发布响应与目标不一致；请核对配置版本，不要重复提交。"};
      return {kind:"acknowledged",publication};
    }catch(cause){
      if(isCancelledError(cause))throw cause;
      const kind=asAppError(cause).kind;
      if(kind==="invalid_request"||kind==="conflict"||kind==="session_invalid")throw cause;
      let observation="发布结果未确认。请核对当前活动配置，不要再次提交同一发布请求。";
      try{
        const current=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:target.id}});
        if(owned(target)&&current.status==="active")observation="目标现已处于活动状态，但无法确认本次发布响应；请核对审计和配置版本，不要再次提交。";
      }catch{ /* A failed read cannot turn an uncertain publish into failure. */ }
      return {kind:"unconfirmed",message:observation};
    }
  },onSuccess:(result,input)=>{
    if(!owned(input.source))return;
    if(result.kind==="unconfirmed")setPanel({kind:"unconfirmed",source:input.source,message:result.message});
    else{
      setPanel({kind:"published",source:input.source,publication:result.publication});
      void queryClient.invalidateQueries({queryKey:["config-versions"]});
      void rereadPublished(result.publication,input.source);
    }
  },onError:cause=>{if(!isCancelledError(cause))setError(asAppError(cause).message);}});
  const finishPublication=()=>{
    if(panel?.kind!=="published"||!panel.summary)return;
    if(owned(panel.source))useVersionStore.getState().select(panel.summary);
    setPanel(undefined);
  };
  const isDraft=context?.status==="draft";
  if(!isDraft&&panel===undefined)return null;
  return <>
    {isDraft&&context?<GlassSurface as="footer" className="dock" material="draft" pane="dock">
      <span>草稿 <span className="idchip"><ResourceIdentity id={context.configVersionId} kind="config" /></span><span className="idchip mono">{context.revision}</span></span>
      {error?<span className="dock-error">{error}</span>:null}
      <span className="dock-actions">
        <button type="button" className="secondary" disabled={validate.isPending||publish.isPending||!!panel} onClick={()=>admission.request(()=>{const target=source();if(!target)return;setError(undefined);setPanel({kind:"validating",source:target});validate.mutate(target);})}>验证</button>
        <button type="button" disabled={validate.isPending||publish.isPending||!!panel} onClick={()=>admission.request(()=>{const target=source();if(!target)return;setError(undefined);setPanel({kind:"confirm",source:target});})}>发布</button>
      </span>
    </GlassSurface>:null}
    {panel?.kind==="confirm"?<LifecycleConfirmation mode="publish" id={panel.source.id} pending={publish.isPending} error={error} onCancel={()=>{if(!publish.isPending)setPanel(undefined);}} onConfirm={(expectedActive,lifecycleEvent,revision)=>publish.mutate({source:panel.source,revision,expectedActive,lifecycleEvent})}/>:null}
    {panel?.kind==="validating"?<Sheet title="正在验证草稿" description="正在核对当前修订号与配置规则。" layout="inspector" busy onEscape={()=>{}} footer={<button type="button" className="secondary" disabled>验证中…</button>}><p role="status">正在验证，请稍候…</p></Sheet>:null}
    {panel?.kind==="validation"?<Sheet title="验证结果" description="校验只针对读取时的草稿修订号，不保留发布权。" layout="inspector" onEscape={()=>setPanel(undefined)} footer={<SheetDismissButton onDismiss={()=>setPanel(undefined)}>关闭</SheetDismissButton>}>
      {panel.result.valid?<p role="status">草稿校验通过。发布前仍会重新核对版本。</p>:<ul>{(panel.result.error_codes??[]).map(code=><li key={code} className="mono">{code}</li>)}</ul>}
      {(!owned(panel.source)||context?.revision!==panel.source.revision)?<p role="alert">草稿已变化，此次校验不能用于当前版本。</p>:null}
    </Sheet>:null}
    {panel?.kind==="published"?<Sheet title="配置已发布" description="发布回执已确认；从服务端重读活动配置后再结束。" layout="inspector" onEscape={finishPublication} footer={panel.summary?<SheetDismissButton onDismiss={finishPublication}>完成</SheetDismissButton>:<><button type="button" className="secondary" onClick={()=>void rereadPublished(panel.publication,panel.source)}>重新读取活动配置</button><SheetDismissButton onDismiss={()=>{setPanel(undefined);navigate("/versions");}}>稍后核对</SheetDismissButton></>}>
      <p role="status">活动版本：<ResourceIdentity id={panel.publication.active_config_version_id} kind="config" /></p>
      {panel.publication.replaced_config_version_id?<p>回滚目标：<ResourceIdentity id={panel.publication.replaced_config_version_id} kind="config" /></p>:null}
      {panel.readError?<p role="alert">发布已确认，但重读失败：{panel.readError}</p>:null}
      {!panel.summary&&!panel.readError?<p role="status">正在重读活动配置…</p>:null}
    </Sheet>:null}
    {panel?.kind==="unconfirmed"?<Sheet title="发布结果待核对" description="响应丢失后不会自动重放发布。" layout="inspector" onEscape={()=>{setPanel(undefined);navigate("/versions");}} footer={<SheetDismissButton onDismiss={()=>{setPanel(undefined);navigate("/versions");}}>查看配置版本</SheetDismissButton>}><p role="alert">{panel.message}</p></Sheet>:null}
  </>;
}
