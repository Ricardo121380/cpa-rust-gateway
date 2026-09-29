import {expect,it} from "vitest";
import {callReceiptIsCurrent,rememberCallReceipt,useCallEvidenceStore,type CallReceipt} from "./callEvidence";
import {useSessionStore} from "../../session/sessionStore";
const receipt:CallReceipt={config_version_id:"active",config_revision:7,credential_id:"account",credential_revision:2,channel_id:"endpoint",route_id:"route",requested_model:"model",protocol:"openai_responses",mode:"json",client_key_id:"key",runtime_build:"build:rust:target",server_instance:"instance",permission_basis:"management_key_projection",outcome:"succeeded",upstream_sent:true,attempt_count:1,observed_at_ms:100};
const system={server_instance:"instance",build:{build_revision:"build",rust_version:"rust",build_target:"target"}};
it("keeps an exact receipt when its dialog closes, and clears it at the session boundary",()=>{
  rememberCallReceipt(receipt);
  expect(useCallEvidenceStore.getState().receipts.account).toEqual(receipt);
  useSessionStore.getState().lock();
  expect(useCallEvidenceStore.getState().receipts.account).toBeUndefined();
});
it("invalidates old success on configuration, authorization, process or build changes",()=>{
  const context={configVersionId:"active",revision:"rev-7"};
  expect(callReceiptIsCurrent(receipt,context,2,system)).toBe(true);
  expect(callReceiptIsCurrent(receipt,{...context,revision:"rev-8"},2,system)).toBe(false);
  expect(callReceiptIsCurrent(receipt,context,3,system)).toBe(false);
  expect(callReceiptIsCurrent(receipt,context,2,{...system,server_instance:"restarted"})).toBe(false);
  expect(callReceiptIsCurrent(receipt,context,2,{...system,build:{...system.build,build_revision:"new"}})).toBe(false);
});
