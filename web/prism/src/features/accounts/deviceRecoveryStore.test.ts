import {beforeEach,expect,it} from "vitest";
import {useDeviceRecoveryStore,type DeviceRecovery} from "./deviceRecoveryStore";
import {useSessionStore} from "../../session/sessionStore";
import type {ConfigurationTask} from "../config-versions/configurationTask";
beforeEach(()=>useDeviceRecoveryStore.setState({entries:[]}));
const task={version:{id:"owned-draft"}} as ConfigurationTask;
const entry:DeviceRecovery={channel:"kiro",sessionId:"original-session",task,id:"requested-account",providerId:"original-owner"};
it("retains only exact non-secret handles and resolves only their task",()=>{
 useDeviceRecoveryStore.getState().retain({...entry,user_code:"excluded",verification_uri:"excluded",secret:"excluded"} as DeviceRecovery);
 expect(Object.keys(useDeviceRecoveryStore.getState().entries[0]!).sort()).toEqual(["channel","credentialId","endpointId","id","providerId","sessionId","task"].sort());
 useDeviceRecoveryStore.getState().resolve(entry.sessionId,{} as ConfigurationTask);expect(useDeviceRecoveryStore.getState().entries).toHaveLength(1);
 useDeviceRecoveryStore.getState().resolve(entry.sessionId,task);expect(useDeviceRecoveryStore.getState().entries).toEqual([]);
});
it("preserves another unresolved session and clears all handles on logout",()=>{
 const store=useDeviceRecoveryStore.getState();store.retain(entry);store.retain({...entry,sessionId:"second"});
 store.resolve(entry.sessionId,task);expect(useDeviceRecoveryStore.getState().entries[0]?.sessionId).toBe("second");
 useSessionStore.getState().lock();expect(useDeviceRecoveryStore.getState().entries).toEqual([]);
});
