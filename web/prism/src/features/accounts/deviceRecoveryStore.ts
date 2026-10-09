import {create} from "zustand";
import type {ConfigurationTask} from "../config-versions/configurationTask";
import {useSessionStore} from "../../session/sessionStore";

export type DeviceRecovery=Readonly<{
  channel:"kiro"|"kimi-coding";sessionId:string;task:ConfigurationTask;
  id:string;providerId:string;endpointId?:string;credentialId?:string;
}>;
export type GrokDeviceRecovery=Readonly<{sessionId:string;name:string;target?:Readonly<{account_id:string;revision:number}>}>;

/** Session-local task ownership survives route/dialog unmounts; never retain device codes,
 * callback URLs, token material, or enrollment responses in this store. */
export const useDeviceRecoveryStore=create<{
  entries:readonly DeviceRecovery[];
  nativeEntries:readonly GrokDeviceRecovery[];
  retainNative:(entry:GrokDeviceRecovery)=>void;
  resolveNative:(sessionId:string)=>void;
  retain:(entry:DeviceRecovery)=>void;
  resolve:(sessionId:string,task:ConfigurationTask)=>void;
}>(set=>({entries:[],nativeEntries:[],
  retainNative:entry=>set(state=>({nativeEntries:[...state.nativeEntries.filter(row=>row.sessionId!==entry.sessionId),{
    sessionId:entry.sessionId,name:entry.name,target:entry.target?{account_id:entry.target.account_id,revision:entry.target.revision}:undefined,
  }].slice(-32)})),
  resolveNative:sessionId=>set(state=>({nativeEntries:state.nativeEntries.filter(row=>row.sessionId!==sessionId)})),
  retain:entry=>set(state=>({entries:[...state.entries.filter(row=>row.sessionId!==entry.sessionId),{
    channel:entry.channel,sessionId:entry.sessionId,task:entry.task,id:entry.id,
    providerId:entry.providerId,endpointId:entry.endpointId,credentialId:entry.credentialId,
  }]})),
  resolve:(sessionId,task)=>set(state=>({entries:state.entries.filter(row=>row.sessionId!==sessionId||row.task!==task)})),
}));
useSessionStore.subscribe((state,previous)=>{
  if(state.generation!==previous.generation)useDeviceRecoveryStore.setState({entries:[],nativeEntries:[]});
});
