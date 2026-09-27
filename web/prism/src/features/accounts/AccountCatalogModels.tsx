import "./accountMetadata.css";
import {useQuery} from "@tanstack/react-query";
import {call} from "../../api/client";
import {PagedReadStatus} from "../../components/PagedReadStatus";
import {useVersionStore} from "../config-versions/versionStore";

type ModelPage=Readonly<{items:readonly {model:string;present_in_last_success:boolean}[];next_cursor:string|null}>;
/** Inline directory evidence stays pinned to this credential, not its provider's other accounts. */
export function AccountCatalogModels({endpointId,credentialId}:{endpointId:string;credentialId:string}) {
  const scope=useVersionStore(state=>state.context?.configVersionId);
  const query=useQuery({queryKey:["account-catalog-models",scope,endpointId,credentialId],enabled:!!scope,retry:false,
    queryFn:({signal})=>call<ModelPage>("listCatalogModels",{query:{endpoint_id:endpointId,credential_id:credentialId,limit:100},signal},{versionScoped:true})});
  const models=query.data?.items.filter(model=>model.present_in_last_success);
  return <><PagedReadStatus query={query}/>{models ? models.length ? <ul className="account-model-list">{models.map(model=><li key={model.model}><code>{model.model}</code></li>)}</ul> : <p>暂无可展示的目录模型，请结合上方观测状态查看完整目录。</p> : null}{query.data?.next_cursor ? <p className="muted">这里展示目录前 100 项，完整结果请进入模型目录。</p> : null}</>;
}
