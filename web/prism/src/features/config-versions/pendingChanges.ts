export type ConfigurationChange=Readonly<{resource_kind:string;resource_key:string;change:"added"|"removed"|"changed";changed_fields:readonly string[]}>;
export type ConfigurationChangePage=Readonly<{base:{id:string;revision:string};target:{id:string;revision:string};items:readonly ConfigurationChange[];next_cursor:string|null}>;
export type ResourceAuditPage=Readonly<{items:readonly {id:string;action:string;actor:string;config_version_id:string;resource_kind:string;resource_id:string;occurred_at_ms:number}[];next_before_id:string|null}>;
const groups:Readonly<Record<string,string>>={upstream:"提供商与账号",endpoint:"提供商与账号",credential:"提供商与账号",credential_binding:"提供商与账号",public_model:"模型与路由",alias:"模型与路由",route:"模型与路由",candidate:"模型与路由",access_group:"客户端访问",route_grant:"客户端访问",client_key:"客户端访问",egress_policy:"出口策略",proxy_pool:"出口策略",proxy_node:"出口策略",egress_binding:"出口策略",routing_price_policy:"路由价格策略"};
const fields:Readonly<Record<string,string>>={name:"名称",status:"状态",enabled:"启用状态",api_format:"接口协议",base_url:"接口地址",priority:"优先级",weight:"权重",concurrency:"并发设置",model_name:"原始模型名",display_name:"显示名称",upstream_id:"提供商",endpoint_id:"接口连接",credential_id:"账号",access_group_id:"访问组",route_id:"路由",public_model_id:"模型",expires_at_ms:"有效期",limits:"历史限制",limits_json:"历史限制",catalog_version_id:"价格目录",comparison:"价格比较规则",policy:"调度策略",max_attempts:"尝试次数",bootstrap_timeout_ms:"启动超时",capability_override_json:"能力配置",encrypted_secret:"已加密的凭据存储内容",secret_id:"凭据存储引用"};
export const changeLabel={added:"新增",removed:"移除",changed:"修改"} as const;
export function changeGroup(kind:string):string{return groups[kind]??"其他资源";}
export function fieldLabel(field:string):string{return fields[field]??field;}
export function groupChanges(rows:readonly ConfigurationChange[]):ReadonlyMap<string,readonly ConfigurationChange[]> {
 const result=new Map<string,ConfigurationChange[]>();for(const row of rows){const group=changeGroup(row.resource_kind);const values=result.get(group)??[];values.push(row);result.set(group,values);}return result;
}
export function assertChangePage(page:ConfigurationChangePage,base:{id:string;revision:string},target:{id:string;revision:string}) {
 if(page.base.id!==base.id||page.base.revision!==base.revision||page.target.id!==target.id||page.target.revision!==target.revision)throw new Error("比较期间配置已变化，请重新读取并比较。");
}
export function independentCatalogEvents(page:ResourceAuditPage,targetId:string){return page.items.filter(event=>event.config_version_id===targetId&&(event.action==="billing_catalog_imported"||event.action==="billing_catalog_rolled_back"));}
