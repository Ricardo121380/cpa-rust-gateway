import { call } from "../../api/client";
import type { RouteListItem, RoutingPage, PublicModel } from "../models/model";
import type { ConfigVersionSummary } from "../config-versions/versionStore";

type Grant={route_id:string;enabled:boolean};
export function summarizePermissions(routes:readonly RouteListItem[],models:readonly PublicModel[],grants:readonly Grant[]) {
  const ids=new Set(grants.filter(grant=>grant.enabled).map(grant=>grant.route_id));
  const matching=routes.filter(route=>ids.has(route.id));
  if(matching.length!==ids.size)throw new Error("部分授权路由未读取，请重新读取权限。");
  const modelIds=[...new Set(matching.map(route=>route.public_model_id))];
  return modelIds.map(id=>{
    const model=models.find(row=>row.id===id);
    if(!model)throw new Error("部分授权模型未读取，请重新读取权限。");
    return {id,name:model.model_name,enabled:model.status==="active"};
  });
}
export async function readKeyPermissionSummaries(source:{id:string;revision:string},groupIds:readonly string[]) {
  const unique=[...new Set(groupIds)];
  if(unique.length>1000)throw new Error("权限摘要超过单次读取范围，请打开密钥编辑查看。");
  const check=async()=>{
    const version=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:source.id}});
    if(version.revision!==source.revision)throw new Error("配置已变化，请重新读取密钥权限。");
  };
  await check();
  const routes:RouteListItem[]=[];let cursor:string|undefined;
  do {
    const page=await call<RoutingPage<RouteListItem>>("listRoutes",{query:{limit:100,...(cursor?{cursor}:{})}},{versionScoped:true});
    if(page.revision!==source.revision)throw new Error("模型配置已变化，请重新读取权限。");
    routes.push(...page.items);cursor=page.next_cursor??undefined;
    if(cursor&&routes.length>=10000)throw new Error("路由超过摘要读取范围，请打开密钥编辑查看。");
  }while(cursor);
  const models=await call<PublicModel[]>("listPublicModels",{},{versionScoped:true});
  const result:Record<string,ReturnType<typeof summarizePermissions>>={};
  // Sequence reads to respect the gateway's bounded blocking-read admission.
  for(const id of unique){
    const grants=await call<Grant[]>("listAccessGroupRoutes",{path:{access_group_id:id}},{versionScoped:true});
    result[id]=summarizePermissions(routes,models,grants);
  }
  await check();return result;
}
