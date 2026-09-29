import {describe,expect,it,vi} from "vitest";
import {continueAuthorizationAfterReview} from "./AuthorizationRecovery";
import type {ConfigurationTask,ConfigurationTaskReview} from "../config-versions/configurationTask";

const review={} as ConfigurationTaskReview;
function recoveredTask(bindings:readonly {credential_id:string;enabled?:boolean}[]=[]) {
  const task={
    version:{id:"draft",status:"draft"},revision:()=>"rev-4",
    acceptReviewedDraft:vi.fn().mockResolvedValue(undefined),
    read:vi.fn().mockImplementation(async(operation:string)=>operation==="getCredential"?{id:"saved",upstream_id:"owner"}:bindings),
    mutate:vi.fn().mockResolvedValue({}),apply:vi.fn(),finish:vi.fn(),
  };
  return {task,configuration:task as unknown as ConfigurationTask};
}
describe("saved authorization recovery",()=>{
  it("repairs a missing binding after the exact review without publishing an unreviewed change",async()=>{
    const {task,configuration}=recoveredTask();
    const result=await continueAuthorizationAfterReview(configuration,review,{upstream_id:"owner",endpoint_id:"endpoint"},"saved");
    expect(task.acceptReviewedDraft).toHaveBeenCalledWith(review);
    expect(task.mutate).toHaveBeenCalledOnce();
    expect(task.mutate).toHaveBeenCalledWith("createEndpointCredentialBinding",{path:{endpoint_id:"endpoint"},body:{credential_id:"saved",enabled:true,priority:0,weight:1,concurrency:1}});
    expect(result).toMatchObject({status:"draft",revision:"rev-4"});
    expect(task.apply).not.toHaveBeenCalled();
    expect(task.finish).not.toHaveBeenCalled();
  });
  it("preserves an existing disabled binding",async()=>{
    const {task,configuration}=recoveredTask([{credential_id:"saved",enabled:false}]);
    await continueAuthorizationAfterReview(configuration,review,{upstream_id:"owner",endpoint_id:"endpoint"},"saved");
    expect(task.mutate).not.toHaveBeenCalled();
  });
  it("keeps binding failure recoverable and does not publish",async()=>{
    const {task,configuration}=recoveredTask();
    task.mutate.mockRejectedValue(new Error("binding rejected"));
    await expect(continueAuthorizationAfterReview(configuration,review,{upstream_id:"owner",endpoint_id:"endpoint"},"saved")).rejects.toThrow("binding rejected");
    expect(task.apply).not.toHaveBeenCalled();
    expect(task.finish).not.toHaveBeenCalled();
  });
  it("rejects changed ownership before attaching the saved account",async()=>{
    const {task,configuration}=recoveredTask();
    task.read.mockResolvedValue({id:"saved",upstream_id:"another-owner"});
    await expect(continueAuthorizationAfterReview(configuration,review,{upstream_id:"owner",endpoint_id:"endpoint"},"saved")).rejects.toThrow("账号目标已变化");
    expect(task.mutate).not.toHaveBeenCalled();
  });
  it("does not read or mutate a draft when its review has become stale",async()=>{
    const {task,configuration}=recoveredTask();
    task.acceptReviewedDraft.mockRejectedValue(new Error("stale review"));
    await expect(continueAuthorizationAfterReview(configuration,review,{upstream_id:"owner",endpoint_id:"endpoint"},"saved")).rejects.toThrow("stale review");
    expect(task.read).not.toHaveBeenCalled();
    expect(task.mutate).not.toHaveBeenCalled();
  });
});
