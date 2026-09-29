import {create} from "zustand";
import {useQuery} from "@tanstack/react-query";
import {call} from "../../api/client";
import {useSessionStore} from "../../session/sessionStore";
import {useVersionStore} from "../config-versions/versionStore";

export type CallReceipt=Readonly<{config_version_id:string;config_revision:number;credential_id:string;credential_revision:number;channel_id:string;route_id:string;requested_model:string;protocol:string;mode:string;client_key_id:string;runtime_build:string|null;server_instance:string;permission_basis:string;outcome:string;upstream_sent:boolean;attempt_count:number;observed_at_ms:number}>;
type SystemIdentity=Readonly<{server_instance:string;build:Readonly<{build_revision:string;rust_version:string;build_target:string}>}>;
export const useCallEvidenceStore=create<{receipts:Readonly<Record<string,CallReceipt>>}>(()=>({receipts:{}}));
/** Latest value-free receipt per account, bounded to this administrator session. No browser storage. */
export function rememberCallReceipt(receipt:CallReceipt):void {
  useCallEvidenceStore.setState(state=>({receipts:Object.fromEntries(Object.entries({...state.receipts,[receipt.credential_id]:receipt}).sort((a,b)=>b[1].observed_at_ms-a[1].observed_at_ms).slice(0,100))}));
}
useSessionStore.subscribe((state,previous)=>{if(state.generation!==previous.generation)useCallEvidenceStore.setState({receipts:{}});});

export function callReceiptIsCurrent(receipt:CallReceipt,context:Readonly<{configVersionId:string;revision:string}>|undefined,credentialRevision:number,system:SystemIdentity):boolean {
  return receipt.server_instance===system.server_instance&&receipt.runtime_build===`${system.build.build_revision}:${system.build.rust_version}:${system.build.build_target}`&&receipt.config_version_id===context?.configVersionId&&`rev-${receipt.config_revision}`===context?.revision&&credentialRevision===receipt.credential_revision;
}
export async function readNativeAccount(id:string,provider:string){
  let cursor:string|undefined;let count=0;const cursors=new Set<string>();
  do{const page=await call<{items:readonly {id:string;provider:string;revision:number;enabled:boolean;auth_status:string}[];next_cursor:string|null}>("listNativeAccounts",{query:{limit:100,...(cursor?{cursor}:{})}});const found=page.items.find(row=>row.id===id&&row.provider===provider);if(found)return found;count+=page.items.length;cursor=page.next_cursor??undefined;if(count>10000||(cursor&&cursors.has(cursor)))throw new Error("原生账号清单未完整读取。");if(cursor)cursors.add(cursor);}while(cursor);throw new Error("原生账号不存在，请重新核对。");
}
/** Validate retained evidence through management reads; never repeat an inference. */
export function useAccountCallEvidence(id:string,nativeProvider?:string,credentialRevision?:number){
  const context=useVersionStore(state=>state.context);
  const generation=useSessionStore(state=>state.generation);
  const receipt=useCallEvidenceStore(state=>state.receipts[id]);
  const current=useQuery({queryKey:["account-call-evidence",generation,receipt?.observed_at_ms,id,credentialRevision,context?.configVersionId,context?.revision],enabled:!!receipt&&!!context,retry:false,refetchOnWindowFocus:true,queryFn:async()=>{
    const credential=nativeProvider?await readNativeAccount(id,nativeProvider):await call<{revision:number}>("getCredential",{path:{credential_id:id}},{versionScoped:true});
    const system=await call<SystemIdentity>("getSystemInformation");
    return !!receipt&&callReceiptIsCurrent(receipt,context,credential.revision,system);
  }});
  return {receipt,current};
}
