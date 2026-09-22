import { useMutation, useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Link } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import type { PublicModel, RoutingPage, RouteListItem } from "../models/model";
import { runModelTask, type ModelTaskReceipt } from "../models/modelTask";
import type { ClientKeyRecord, IssuedClientKey } from "./model";

export function IssueKeyDialog({onClose,onSaved}:Readonly<{onClose:()=>void;onSaved:(version:ConfigVersionSummary)=>void}>){
  const formId="issue-key-form";
  const [owner]=useState(()=>useVersionStore.getState().context);
  const scope=owner?.configVersionId;
  const models=useQuery({queryKey:["public-models",scope,owner?.revision],queryFn:()=>call<PublicModel[]>("listPublicModels",{},{versionScoped:true}),enabled:!!scope});
  const [name,setName]=useState("");
  const [search,setSearch]=useState("");
  const [chosen,setChosen]=useState<ReadonlySet<string>>(new Set());
  const [days,setDays]=useState("30");
  const [issued,setIssued]=useState<IssuedClientKey>();
  const [receipt,setReceipt]=useState<ModelTaskReceipt>();
  const [recovery,setRecovery]=useState<string>();
  const [copied,setCopied]=useState(false);
  const [copyError,setCopyError]=useState(false);
  const submitted=useRef(false);
  useEffect(()=>useSessionStore.subscribe((state,previous)=>{
    if(state.generation!==previous.generation)setIssued(undefined);
  }),[]);
  const dirty=name.trim().length>0||chosen.size>0||days!=="30";
  const save=useMutation({gcTime:0,mutationFn:async()=>{
    if(!owner||!chosen.size)throw new Error("请选择允许使用的模型。");
    const selectedModels=new Set(chosen);
    const label=name.trim();
    const keyId=`key-${crypto.randomUUID()}`;
    let returned=false;
    const result=await runModelTask(`创建客户端密钥 · ${label}`,async task=>{
      const routes:RouteListItem[]=[];let cursor:string|undefined;let revision:string|undefined;
      do{
        const page=await task.read<RoutingPage<RouteListItem>>("listRoutes",{query:{limit:100,...(cursor?{cursor}:{})}});
        if(revision&&revision!==page.revision)throw new Error("模型配置已变化，请重新选择模型。");
        revision=page.revision;routes.push(...page.items);cursor=page.next_cursor??undefined;
        if(routes.length>=10000&&cursor)throw new Error("配置中的路由超过本次操作范围，请使用高级访问组配置。");
      }while(cursor);
      const available=await task.read<PublicModel[]>("listPublicModels");
      if([...selectedModels].some(id=>!available.some(model=>model.id===id&&model.status==="active")))throw new Error("所选模型已关闭或变化，请重新选择。");
      const selected=routes.filter(route=>selectedModels.has(route.public_model_id));
      if([...selectedModels].some(id=>!selected.some(route=>route.public_model_id===id)))throw new Error("选中的模型尚未配置接口连接。");
      const group=`group-${crypto.randomUUID()}`;
      await task.mutate("createAccessGroup",{body:{id:group,name:label,status:"active",limits:{}}});
      for(const route of selected)await task.mutate("grantAccessGroupRoute",{path:{access_group_id:group},body:{route_id:route.id,enabled:true}});
      const credential=await task.mutate<IssuedClientKey>("issueClientKey",{body:{id:keyId,access_group_id:group,status:"active",expires_at_ms:days==="0"?null:Date.now()+Number(days)*86400000}});
      returned=true;
      setIssued(credential);
    },{expectedSource:{id:owner.configVersionId,revision:owner.revision},probeUnchanged:true});
    let evidence:string|undefined;
    if(result.receipt.kind==="unconfirmed"&&!returned){
      const generation=useSessionStore.getState().generation;
      const selection=useVersionStore.getState().selectionGeneration;
      try{
        const keys=await call<ClientKeyRecord[]>("listClientKeys",{headers:{"X-Config-Version":result.receipt.workingVersion.id}});
        if(generation===useSessionStore.getState().generation&&selection===useVersionStore.getState().selectionGeneration){
          evidence=keys.some(key=>key.id===keyId)?"密钥记录已存在，但完整密钥无法再次读取。请撤销该记录并重新签发。":"未确认密钥是否生成。请先核对工作配置中的密钥记录，不要重复提交。";
        }
      }catch{
        evidence="密钥记录暂时无法重读；完整密钥不可恢复，请核对后决定是否撤销并重签。";
      }
    }
    return {receipt:result.receipt,evidence};
  },onSuccess:({receipt:next,evidence})=>{setReceipt(next);setRecovery(evidence);},onError:()=>{submitted.current=false;}});
  const done=()=>{
    if(save.isPending)return;
    setIssued(undefined);
    save.reset();
    if(receipt&&receipt.kind!=="unchanged")onSaved(receipt.workingVersion);
    else onClose();
  };
  const submit=(event:FormEvent)=>{event.preventDefault();if(submitted.current)return;submitted.current=true;save.mutate();};
  const copy=()=>{if(!issued)return;void navigator.clipboard.writeText(issued.key).then(()=>{setCopied(true);setCopyError(false);},()=>setCopyError(true));};
  const resultLabel=issued?(receipt?.kind==="saved_applied"?"完成并清除密钥":"核对配置并清除密钥"):receipt?.kind==="saved_applied"?"完成":"核对配置";
  return <Sheet title={issued?"API 密钥已生成":receipt?"密钥签发结果":"创建 API 密钥"} description={issued?"完整密钥只在此处显示一次；离开后无法恢复。":receipt?"请先核对配置与密钥记录，再决定是否重新操作。":"选择可调用的已开放模型，并设置密钥有效期。"} onEscape={done} busy={save.isPending} isDirty={!receipt&&!issued&&dirty} footer={receipt||issued?<><button type="button" className="secondary" disabled={!issued} onClick={copy}>{copied?"已复制":"复制密钥"}</button><SheetDismissButton onDismiss={done} disabled={save.isPending}>{resultLabel}</SheetDismissButton></>:<><SheetDismissButton className="secondary" disabled={save.isPending}>取消</SheetDismissButton><button type="submit" form={formId} disabled={!chosen.size||submitted.current}>{save.isPending?"正在签发…":"创建并应用"}</button></>}>
    {issued?<><p role="status">{save.isPending?"密钥已生成，正在应用配置…":receipt?.kind==="saved_applied"?"密钥已生效，可以连接客户端。":"密钥已生成，但配置尚未确认应用。"}</p><code className="reveal-key mono">{issued.key}</code><p className="muted">请现在复制并安全保存完整密钥。</p>{copyError?<p role="alert">无法访问剪贴板，请手动复制。</p>:null}</>:receipt?<div role="status"><p>{receipt.message}</p>{recovery?<p>{recovery}</p>:null}<p>已确认保存 {receipt.acknowledgedWrites} 步。</p></div>:<form id={formId} className="sheet-form key-permissions-form key-issue-form" onSubmit={submit}>
      <div className="key-fields-row"><label>名称<input required maxLength={256} value={name} disabled={submitted.current} onChange={event=>setName(event.target.value)} placeholder="例如：我的客户端"/></label>
      <label>有效期<select value={days} onChange={event=>setDays(event.target.value)} disabled={submitted.current}><option value="30">30 天</option><option value="90">90 天</option><option value="365">1 年</option><option value="0">不过期</option></select></label></div>
      <fieldset className="key-model-picker" disabled={submitted.current}><legend>允许使用的模型 · 已选 {chosen.size}</legend><label>搜索模型<input type="search" value={search} onChange={event=>setSearch(event.target.value)} placeholder="按原始模型 ID 搜索"/></label><div className="data-toolbar"><button type="button" className="secondary" onClick={()=>setChosen(new Set(models.data?.filter(model=>model.status==="active").map(model=>model.id)??[]))}>全选当前已开放模型</button><button type="button" className="secondary" onClick={()=>setChosen(new Set())}>清空选择</button></div><div className="key-model-options">{models.data?.filter(model=>model.status==="active"&&model.model_name.toLowerCase().includes(search.trim().toLowerCase())).map(model=><label className="check-row" key={model.id}><input type="checkbox" checked={chosen.has(model.id)} onChange={()=>setChosen(current=>{const next=new Set(current);if(next.has(model.id))next.delete(model.id);else next.add(model.id);return next;})}/><span className="mono">{model.model_name}</span></label>)}</div><p className="field-help">新增模型不会自动加入此密钥。搜索不会改变已选权限。</p></fieldset>
      {models.isError?<p role="alert">{asAppError(models.error).message}</p>:null}
      {!models.data?.some(model=>model.status==="active")?<Link to="/models?add=model">先开放模型</Link>:null}
    </form>}
    {save.isError?<p role="alert">{asAppError(save.error).message}</p>:null}
    {receipt&&issued?<p role="status">{receipt.message}{recovery?` ${recovery}`:""}</p>:null}
  </Sheet>;
}
