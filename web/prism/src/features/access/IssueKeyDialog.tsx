import { useMutation, useQuery } from "@tanstack/react-query";
import { useRef, useState, type FormEvent } from "react";
import { Link } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import { ConfigurationTaskNotice } from "../config-versions/ConfigurationTaskNotice";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import type { PublicModel, RoutingPage, RouteListItem } from "../models/model";
import type { IssuedClientKey } from "./model";

export function IssueKeyDialog({onClose,onSaved}:Readonly<{onClose:()=>void;onSaved:(version:ConfigVersionSummary)=>void}>) {
  const scope=useVersionStore((state)=>state.context?.configVersionId);
  const models=useQuery({queryKey:["public-models",scope],queryFn:()=>call<PublicModel[]>("listPublicModels",{},{versionScoped:true}),enabled:!!scope});
  const [name,setName]=useState("");
  const [chosen,setChosen]=useState<ReadonlySet<string>>(new Set());
  const [days,setDays]=useState("30");
  const [workingId,setWorkingId]=useState<string>();
  const [issued,setIssued]=useState<IssuedClientKey>();
  const [copied,setCopied]=useState(false);
  const [copyError,setCopyError]=useState(false);
  const submitted=useRef(false);
  const save=useMutation({gcTime:0,mutationFn:async()=>{
    if(!chosen.size||chosen.size>20)throw new Error("请选择 1–20 个模型。");
    const task=await beginConfigurationTask(`创建客户端密钥 · ${name.trim()}`);setWorkingId(task.version.id);
    // Re-read every bounded route page in the working revision before granting any access.
    const routes:RouteListItem[]=[];let cursor:string|undefined;let revision:string|undefined;
    do {
      const page=await task.read<RoutingPage<RouteListItem>>("listRoutes",{query:{limit:100,...(cursor?{cursor}:{})}});
      if(revision&&revision!==page.revision)throw new Error("模型配置已变化，请重新选择模型。");
      revision=page.revision;routes.push(...page.items);cursor=page.next_cursor??undefined;
      if(routes.length>=10000&&cursor)throw new Error("配置中的路由超过本次操作范围，请使用高级访问组配置。");
    } while(cursor);
    const selected=routes.filter((route)=>chosen.has(route.public_model_id));
    if([...chosen].some((id)=>!selected.some((route)=>route.public_model_id===id)))throw new Error("选中的模型尚未配置接口连接。");
    if(selected.length>100)throw new Error("所选模型包含超过 100 条路由，请缩小范围。");
    const group=`group-${crypto.randomUUID()}`;
    await task.mutate("createAccessGroup",{body:{id:group,name:name.trim(),status:"active",limits:{}}});
    for(const route of selected)await task.mutate("grantAccessGroupRoute",{path:{access_group_id:group},body:{route_id:route.id,enabled:true}});
    const credential=await task.mutate<IssuedClientKey>("issueClientKey",{body:{id:`key-${crypto.randomUUID()}`,access_group_id:group,status:"active",expires_at_ms:days==="0"?null:Date.now()+Number(days)*86400000}});
    setIssued(credential);
    return task.finish();
  }});
  const submit=(event:FormEvent)=>{event.preventDefault();if(submitted.current)return;submitted.current=true;save.mutate();};
  const close=()=>{if(save.isPending)return;setIssued(undefined);if(save.data)onSaved(save.data);else onClose();};
  return <Sheet title={issued?"客户端密钥":"创建客户端密钥"} onEscape={close}>
    {issued?<><p role="status">{save.isPending?"密钥已生成，正在应用配置…":save.data?.status==="active"?"已生效，可以连接客户端。":"密钥已生成，配置尚未应用。"}</p><code className="reveal-key mono">{issued.key}</code><p className="muted">完整密钥只显示这一次，请妥善保存。</p><button className="secondary" onClick={()=>void navigator.clipboard.writeText(issued.key).then(()=>{setCopied(true);setCopyError(false);},()=>setCopyError(true))}>{copied?"已复制":"复制密钥"}</button>{copyError?<p role="alert">无法访问剪贴板，请手动复制。</p>:null}<div className="sheet-actions"><button disabled={save.isPending} onClick={close}>完成</button></div></>:<form className="sheet-form" onSubmit={submit}>
      <label>名称<input required maxLength={256} value={name} disabled={submitted.current} onChange={(event)=>setName(event.target.value)} placeholder="例如：我的客户端"/></label>
      <label>有效期<select value={days} onChange={(event)=>setDays(event.target.value)} disabled={submitted.current}><option value="30">30 天</option><option value="90">90 天</option><option value="365">1 年</option><option value="0">不过期</option></select></label>
      <fieldset disabled={submitted.current}><legend>允许使用的模型 · 已选 {chosen.size} / 20</legend>{models.data?.filter((model)=>model.status==="active").map((model)=><label className="check-row" key={model.id}><input type="checkbox" checked={chosen.has(model.id)} disabled={!chosen.has(model.id)&&chosen.size>=20} onChange={()=>setChosen((current)=>{const next=new Set(current);if(next.has(model.id))next.delete(model.id);else next.add(model.id);return next;})}/>{model.model_name}</label>)}</fieldset>
      {models.isError?<p role="alert">{asAppError(models.error).message}</p>:null}
      {!models.data?.some((model)=>model.status==="active")?<Link to="/models?add=model" onClick={onClose}>先开放模型</Link>:null}
      <div className="sheet-actions"><button type="button" className="secondary" disabled={save.isPending} onClick={onClose}>取消</button><button type="submit" disabled={!chosen.size||submitted.current}>创建并应用</button></div>
    </form>}
    <ConfigurationTaskNotice workingId={workingId} error={save.error} onReview={(version)=>{setIssued(undefined);onSaved(version);}}/>
  </Sheet>;
}
