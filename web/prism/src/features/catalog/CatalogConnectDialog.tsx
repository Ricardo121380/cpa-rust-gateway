import { CancelledError, useMutation } from "@tanstack/react-query";
import { useRef, useState } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import type { ConfigVersionSummary } from "../config-versions/versionStore";
import { useVersionStore } from "../config-versions/versionStore";
import { useSessionStore } from "../../session/sessionStore";
import { connectModelBatch, type ModelBatchRow } from "../models/connectModelBatch";
import { runModelTask, type ModelTaskReceipt } from "../models/modelTask";
import type { ManagedEndpoint } from "../accounts/inventory";

export type CatalogConnectionSelection=Readonly<{
  configVersion:string;
  configRevision:string;
  endpointId:string;
  endpoint:ManagedEndpoint;
  credentialId:string;
  accountName:string;
  snapshotVersion:number;
  observedAtMs:number;
  expiresAtMs:number;
  models:readonly string[];
}>;

type Header=Readonly<{revision:string;target:{endpoint_id:string;credential_id:string;snapshot_version:number;observed_at_ms:number;expires_at_ms:number}}>;
const endpointFields=["id","upstream_id","adapter_id","api_format","base_url","inference_path","models_path","transport","enabled"] as const;
const sameEndpoint=(left:ManagedEndpoint,right:ManagedEndpoint)=>endpointFields.every(field=>left[field]===right[field]);

export function CatalogConnectDialog({selection,onClose,onSaved}:Readonly<{selection:CatalogConnectionSelection;onClose:()=>void;onSaved:(version:ConfigVersionSummary)=>void}>) {
  const [rows,setRows]=useState<readonly ModelBatchRow[]>(selection.models.map(model=>({model,state:"waiting"})));
  const [receipt,setReceipt]=useState<ModelTaskReceipt>();
  const submitted=useRef(false);
  const save=useMutation({mutationFn:async()=>{
    const owner=useVersionStore.getState().selectionGeneration;
    const session=useSessionStore.getState().generation;
    const context=useVersionStore.getState().context;
    if(context?.configVersionId!==selection.configVersion||context.revision!==selection.configRevision)throw new Error("当前配置已变化，请重新选择目录模型。");
    if(Date.now()>=selection.expiresAtMs)throw new Error("目录证据已过期，请刷新上游模型后重新选择。");
    const latest=await call<Header>("listCatalogModels",{query:{endpoint_id:selection.endpointId,credential_id:selection.credentialId,limit:1}},{versionScoped:true});
    if(owner!==useVersionStore.getState().selectionGeneration||session!==useSessionStore.getState().generation)throw new CancelledError({silent:true});
    if(latest.revision!==selection.configRevision||latest.target.endpoint_id!==selection.endpointId||latest.target.credential_id!==selection.credentialId||latest.target.snapshot_version!==selection.snapshotVersion||latest.target.observed_at_ms!==selection.observedAtMs||Date.now()>=latest.target.expires_at_ms)throw new Error("目录已更新或过期，请重读后重新选择。");
    return runModelTask(`从上游目录接入 ${selection.models.length} 个模型`,async task=>{
      const workingEndpoint=await task.read<ManagedEndpoint>("getEndpoint",{path:{endpoint_id:selection.endpointId}});
      if(!sameEndpoint(workingEndpoint,selection.endpoint))throw new Error("所选接口已变化，请重新核对目录与账号。");
      const workingCatalog=await task.read<Header>("listCatalogModels",{query:{endpoint_id:selection.endpointId,credential_id:selection.credentialId,limit:1}});
      if(workingCatalog.target.endpoint_id!==selection.endpointId||workingCatalog.target.credential_id!==selection.credentialId||workingCatalog.target.snapshot_version!==selection.snapshotVersion||workingCatalog.target.observed_at_ms!==selection.observedAtMs||Date.now()>=workingCatalog.target.expires_at_ms)throw new Error("工作配置中的目录证据已变化，请重新选择。");
      return connectModelBatch(task,selection.models,selection.endpointId,setRows);
    },{expectedSource:{id:selection.configVersion,revision:selection.configRevision}});
  },onSuccess:({receipt:next})=>setReceipt(next),onError:()=>{submitted.current=false;}});
  const done=()=>{if(receipt&&receipt.kind!=="unchanged")onSaved(receipt.workingVersion);else onClose();};
  const states:Record<ModelBatchRow["state"],string>={waiting:"未执行",saving:"正在保存",saved:"已保存，待应用",existing:"来源已存在",uncertain:"需核对"};
  return <Sheet title={receipt?"模型接入结果":"确认接入上游模型"} description="本次选择只使用该账号的目录证据；运行调度仍按已配置的可用账号池进行。" layout="confirm" onEscape={()=>!save.isPending&&done()} busy={save.isPending} footer={receipt?<SheetDismissButton onDismiss={done}>{receipt.kind==="saved_applied"||receipt.kind==="unchanged"?"完成":"核对配置"}</SheetDismissButton>:<><SheetDismissButton className="secondary" disabled={save.isPending}>取消</SheetDismissButton><button type="button" disabled={save.isPending||submitted.current} onClick={()=>{if(submitted.current)return;submitted.current=true;save.mutate();}}>确认接入</button></>}>
    <p><strong>{selection.accountName}</strong> · 目录观测于 {new Date(selection.observedAtMs).toLocaleString()}</p>
    <p>模型 ID 将原样进入配置；本次共 {selection.models.length} 个。</p>
    <ul className="model-batch-results">{rows.map(row=><li key={row.model}><code>{row.model}</code><span>{receipt?.kind==="saved_applied"&&row.state==="saved"?"已应用":states[row.state]}</span></li>)}</ul>
    {receipt?<p role="status">{receipt.message} 已确认保存 {receipt.acknowledgedWrites} 步。</p>:save.isError?<p role="alert">{asAppError(save.error).message}</p>:null}
  </Sheet>;
}
