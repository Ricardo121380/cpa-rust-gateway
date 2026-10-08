import {useState} from "react";
import {KiroDeviceDialog} from "./KiroDeviceDialog";
import {KimiDeviceDialog} from "./KimiDeviceDialog";
import {useDeviceRecoveryStore,type DeviceRecovery} from "./deviceRecoveryStore";

export function DeviceRecoveryHost(){
  const entries=useDeviceRecoveryStore(state=>state.entries);
  const [opened,setOpened]=useState<DeviceRecovery>();
  const close=()=>setOpened(undefined);
  return <>
    {entries.map(entry=>{const label=entry.channel==="kiro"?"Kiro":"Kimi Coding";
      return <div className="workspace-context" role="status" key={entry.sessionId}>
        <span>{label} 授权结果待确认 · 原任务 {entry.task.version.id}。返回后只读取原会话结果。</span>
        <button type="button" className="secondary" onClick={()=>setOpened(entry)}>返回 {label} 授权任务</button>
      </div>;
    })}
    {opened?.channel==="kiro"?<KiroDeviceDialog recovery={opened} credentialId={opened.credentialId} onClose={close} onComplete={close}/>:null}
    {opened?.channel==="kimi-coding"?<KimiDeviceDialog recovery={opened} credentialId={opened.credentialId} onClose={close} onComplete={close}/>:null}
  </>;
}
