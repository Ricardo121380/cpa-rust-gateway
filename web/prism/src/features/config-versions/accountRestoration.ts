import {call} from "../../api/client";
import type {ConfigVersionSummary} from "./versionStore";
export type AccountRestorationReview=Readonly<{target_id:string;target_revision:number;active_id:string|null;active_revision:number|null;deletion_event_id:number;review_token:string;accounts:readonly Readonly<{credential_id:string;upstream_id:string;credential_revision:number}>[]}>;
export async function readAccountRestorationReview(target:ConfigVersionSummary,activeId:string|null){
  const review=await call<AccountRestorationReview>("getAccountRestorationReview",{path:{config_version_id:target.id}});
  if(review.target_id!==target.id||`rev-${review.target_revision}`!==target.revision||review.active_id!==activeId||!Array.isArray(review.accounts)||!/^[a-f0-9]{64}$/u.test(review.review_token))throw new Error("账号恢复清单已变化，请重新核对。");
  return review;
}
export function restorationHeaders(review:AccountRestorationReview|undefined,confirmed:boolean):Record<string,string>{
  if(!review?.accounts.length)return {};
  if(!confirmed)throw new Error("请明确确认将恢复的已删除账号及其授权 revision。");
  return {"X-Account-Restoration-Review":review.review_token};
}
