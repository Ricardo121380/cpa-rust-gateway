import {create} from "zustand";
import type {ConfigurationTask} from "../config-versions/configurationTask";
import {useSessionStore} from "../../session/sessionStore";

export type DeviceRecovery=Readonly<{
  channel:"kiro"|"kimi-coding";sessionId:string;task:ConfigurationTask;
  id:string;providerId:string;endpointId?:string;credentialId?:string;
}>;

/** Session-local task ownership survives route/dialog unmounts; never retain device codes,
 * callback URLs, token material, or enrollment responses in this store. */
export const useDeviceRecoveryStore=create<{
  entries:readonly DeviceRecovery[];
  retain:(entry:DeviceRecovery)=>void;
  resolve:(sessionId:string,task:ConfigurationTask)=>void;
}>(set=>({entries:[],
  retain:entry=>set(state=>({entries:[...state.entries.filter(row=>row.sessionId!==entry.sessionId),{
    channel:entry.channel,sessionId:entry.sessionId,task:entry.task,id:entry.id,
    providerId:entry.providerId,endpointId:entry.endpointId,credentialId:entry.credentialId,
  }]})),
  resolve:(sessionId,task)=>set(state=>({entries:state.entries.filter(row=>row.sessionId!==sessionId||row.task!==task)})),
}));
useSessionStore.subscribe((state,previous)=>{
  if(state.generation!==previous.generation)useDeviceRecoveryStore.setState({entries:[]});
});
