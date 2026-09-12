import type { beginConfigurationTask } from "../config-versions/configurationTask";
import type { PublicModel } from "./model";

export type ConfigurationTask=Awaited<ReturnType<typeof beginConfigurationTask>>;
export type ModelConnection=Readonly<{model:string;upstreamModel:string;endpointId:string;allowUnlisted:boolean}>;

export async function connectModel(task:ConfigurationTask,input:ModelConnection) {
  const models=await task.read<PublicModel[]>("listPublicModels");
  if(models.some((row)=>row.model_name===input.model))throw new Error("这个模型名称已存在，请在现有模型中管理连接或使用其他名称。");
  const id=`model-${crypto.randomUUID()}`,route=`route-${crypto.randomUUID()}`;
  await task.mutate("createPublicModel",{body:{id,model_name:input.model,display_name:input.model,status:"active",capabilities:{streaming:true}}});
  await task.mutate("createRoute",{path:{public_model_id:id},body:{id:route,policy:"smooth_weighted_round_robin",max_attempts:1,bootstrap_timeout_ms:30000}});
  await task.mutate("createRouteCandidate",{path:{route_id:route},body:{id:`candidate-${crypto.randomUUID()}`,endpoint_id:input.endpointId,upstream_model:input.upstreamModel,credential_scope:"all_active",transform_mode:"canonical",enabled:true,priority:0,weight:1,capability_override:input.allowUnlisted?{allow_unlisted_model:true}:{}}});
  return {id,route};
}
