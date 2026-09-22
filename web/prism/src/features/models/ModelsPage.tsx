import { modelSources } from "./modelSources";
import { WorkspaceTabs } from "../../app/WorkspaceTabs";
import { useModelWorkspaceFilters } from "./workspaceFilters";
import { useOperationBoundary } from "../../components/OperationBoundary";
import { Link, useNavigate } from "react-router-dom";
import { useModelConnections } from "./useModelConnections";
import { ModelConnectionsDialog } from "./ModelConnectionsDialog";
import { protocolName } from "../accounts/presentation";
import { resourceName } from "../../utils/resourceNames";
import { useSearchParams } from "react-router-dom";
import { ReadStatus } from "../../components/ReadStatus";
// Public models: client-visible model names + capabilities + 1:1 route.
// Complete route/candidate/alias enumeration lives in RouteWorkbench.
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { flushSync } from "react-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { ObjectInspector } from "../../components/ObjectInspector";
import { StatusBadge } from "../../components/StatusBadge";
import { useMessages } from "../../i18n/messages";
import { useVersionStore } from "../config-versions/versionStore";
import { ConnectModelDialog } from "./ConnectModelDialog";
import { ModelEditorDialog } from "./ModelEditorDialog";
import { ModelDeleteDialog } from "./ModelDeleteDialog";
import { ModelAliasDialog } from "./ModelAliasDialog";
import { RouteCreateDialog } from "./RouteCreateDialog";
import { captureModelActionOwner, isModelActionOwner, type DraftRoutingOwner } from "./advancedRoutingTask";
import type { ConfigVersionSummary } from "../config-versions/versionStore";
import type { ModelTaskReceipt } from "./modelTask";
import "./models.css";
import { RouteWorkbench } from "./RouteWorkbench";
import {
  enabledCapabilities,
  type PublicModel,
} from "./model";

export function ModelsPage() {
  const boundary=useOperationBoundary();
  const navigate=useNavigate();
  const [search, setSearch] = useSearchParams();
  const sourceModel = search.get("from_model") ?? "";
  const sourceEndpoint = search.get("from_endpoint") ?? "";
  const modelSeed =
    sourceModel.length > 0 &&
    sourceModel.length <= 256 &&
    sourceEndpoint.length > 0 &&
    sourceEndpoint.length <= 128
      ? { model: sourceModel, endpoint: sourceEndpoint }
      : undefined;
  const t = useMessages();
  const queryClient = useQueryClient();
  const context = useVersionStore((s) => s.context);
  const editable = context?.status === "draft";
  const scope = context?.configVersionId;
  const [modelEditor,setModelEditor]=useState<{initial?:PublicModel;owner:DraftRoutingOwner}>();
  const [inspected, setInspected] = useState<PublicModel>();
  const [connectionTarget,setConnectionTarget]=useState<PublicModel>();
  const searchText=useModelWorkspaceFilters(state=>state.modelSearch);
  const setSearchText=useModelWorkspaceFilters(state=>state.setModelSearch);
  const [connectionSeed,setConnectionSeed]=useState<{model:string;endpoint:string;alias?:string;targetModelId?:string}>();
  const providerFilter=useModelWorkspaceFilters(state=>state.modelProvider);
  const setProviderFilter=useModelWorkspaceFilters(state=>state.setModelProvider);
  const topology=useModelConnections();
  const providers=useQuery({queryKey:["upstreams",scope,context?.revision],queryFn:()=>call<{id:string;name:string}[]>("listUpstreams",{},{versionScoped:true}),enabled:!!scope});
  const [confirmDelete, setConfirmDelete] = useState<{model:PublicModel;owner:DraftRoutingOwner}>();
  const [aliasTarget, setAliasTarget] = useState<{model:PublicModel;owner:DraftRoutingOwner}>();
  const [routeTarget, setRouteTarget] = useState<{model:PublicModel;owner:DraftRoutingOwner}>();
  const [candidateActive,setCandidateActive]=useState(false);
  const [createdRouteId, setCreatedRouteId] = useState<string | undefined>();
  const [notice, setNotice] = useState<string | undefined>();
  const [connecting,setConnecting]=useState(search.get("add")==="model");
  const closeConnecting=()=>{setConnecting(false);const next=new URLSearchParams(search);next.delete("add");setSearch(next,{replace:true});};
  const [actionError, setActionError] = useState<string | undefined>();

  const models = useQuery({
    queryKey: ["public-models", scope, context?.revision],
    queryFn: () => call<PublicModel[]>("listPublicModels", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });

  const connectionsFor=(id:string)=>modelSources(id,topology.data?.routes??[],topology.data?.candidates??[]);
  const visibleModels=(models.data??[]).filter(model=>
    model.model_name.toLowerCase().includes(searchText.trim().toLowerCase())&&
    (!providerFilter||connectionsFor(model.id).some(candidate=>topology.data?.endpoints.some(endpoint=>endpoint.id===candidate.endpoint_id&&endpoint.upstream_id===providerFilter)))
  );
  const filteredSourcesReady=!providerFilter||!!topology.data&&!topology.isError;

  const invalidate = () => {
    void queryClient.resetQueries({ queryKey: ["public-models"] });
    void queryClient.resetQueries({ queryKey: ["model-connections"] });
    void queryClient.resetQueries({ queryKey: ["routing-inventory", scope] });
  };

  const openModelEditor=(initial?:PublicModel)=>boundary.request(()=>{
    try{setModelEditor({initial,owner:captureModelActionOwner()});setActionError(undefined);}
    catch(cause){setActionError(asAppError(cause).message);}
 });
  const openModelDelete=(model:PublicModel)=>boundary.request(()=>{
    try{setConfirmDelete({model,owner:captureModelActionOwner()});setActionError(undefined);}
    catch(cause){setActionError(asAppError(cause).message);}
 });
  const openModelAliases=(model:PublicModel)=>boundary.request(()=>{
    try{setAliasTarget({model,owner:captureModelActionOwner()});setActionError(undefined);}
    catch(cause){setActionError(asAppError(cause).message);}
 });
  const openRouteCreate=(model:PublicModel)=>boundary.request(()=>{
    try{const owner=captureModelActionOwner();if(context?.status!=="draft")throw new Error("请先选择草稿后配置路由。");setRouteTarget({model,owner});setActionError(undefined);}
    catch(cause){setActionError(asAppError(cause).message);}
 });
  const settleModelReceipt=(receipt:ModelTaskReceipt)=>{
    if(receipt.kind==="unconfirmed")return;
    if(receipt.kind!=="unchanged"){
      const store=useVersionStore.getState();
      if(store.context?.configVersionId===receipt.workingVersion.id&&store.context.status===receipt.workingVersion.status)store.advanceFromEtag(receipt.workingVersion.revision);
      else store.select(receipt.workingVersion);
    }
    invalidate();
  };
  const reviewModelReceipt=async(receipt:ModelTaskReceipt,owner:DraftRoutingOwner,close:()=>void)=>{
    if(!isModelActionOwner(owner))throw new Error("当前会话或配置已变化，请重新登录后核对工作配置。");
    const version=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:receipt.workingVersion.id}});
    if(!isModelActionOwner(owner))throw new Error("当前会话或配置已变化，请重新登录后核对工作配置。");
    if(version.id!==receipt.workingVersion.id)throw new Error("重读返回的配置与本次工作配置不一致。");
    // Retire the busy Sheet's navigation blocker before moving to the exact draft.
    flushSync(close);
    useVersionStore.getState().select(version);
    invalidate();
    navigate("/versions");
  };
  const finishModelEditor=(receipt:ModelTaskReceipt)=>{setModelEditor(undefined);settleModelReceipt(receipt);};

  return (
    <section className="models-page">
      <header className="page-head">
        <div><h2>{t.nav.models}</h2><p className="page-description">上游发现、对外开放与当前可用性，是三个不同状态。</p></div>
        <div className="page-actions">
          <button onClick={()=>boundary.request(()=>{setConnectionSeed(undefined);setConnecting(true);})}>接入模型</button>

        </div>
      </header>
      <WorkspaceTabs />
      {connecting?<ConnectModelDialog targetModelId={connectionSeed?.targetModelId} endpointSeed={search.get("from_endpoint")??undefined} seed={connectionSeed??modelSeed} onClose={closeConnecting} onSaved={(version)=>{closeConnecting();useVersionStore.getState().select(version);invalidate();}}/>:null}

      {modelSeed !== undefined ? (
        <section className="data-panel data-panel--padded" aria-label="待用于草稿的模型">
          <h3>已选择授权模型</h3>
          <p className="mono">
            {modelSeed.model}
          </p>
          <p className="stat-sub">
            已选择模型与接口，保存时会校验最新的账号目录。
          </p>
          <button
            className="secondary"
            onClick={() => boundary.request(()=>setConnecting(true))}
          >
            开放此模型
          </button>
          <button
            className="secondary"
            onClick={() => {
              const next = new URLSearchParams(search);
              for (const name of [
                "from_model",
                "from_endpoint",
                "from_version",
              ])
                next.delete(name);
              setSearch(next, { replace: true });
            }}
          >
            清除模型选择
          </button>
        </section>
      ) : null}
      {notice !== undefined ? (
        <p className="action-notice">
          {notice}
          <button type="button" onClick={() => setNotice(undefined)}>
            知道了
          </button>
        </p>
      ) : null}
      {actionError !== undefined ? (
        <p role="alert" className="action-error">
          {actionError}
          <button type="button" onClick={() => setActionError(undefined)}>
            清除
          </button>
        </p>
      ) : null}

      <ReadStatus pending={!!scope&&models.isPending} error={models.error} hasData={models.data !== undefined} retry={() => void models.refetch()} />

      <div className="model-source-context"><div><strong>{providerFilter?resourceName(providerFilter,"upstream",providers.data?.find(provider=>provider.id===providerFilter)?.name):"全部来源"}</strong><span>原始模型 ID 与提供商来源同时保留</span></div>{providerFilter?<button className="secondary" onClick={()=>setProviderFilter("")}>清除来源筛选</button>:<Link to="/catalog">浏览上游目录</Link>}</div>
      <div className="data-toolbar model-filters"><input type="search" aria-label="搜索已接入模型" placeholder="搜索模型 ID" value={searchText} onChange={e=>setSearchText(e.target.value)}/><select aria-label="模型提供商" value={providerFilter} disabled={providers.isPending||providers.isError} onChange={event=>setProviderFilter(event.target.value)}><option value="">全部提供商</option>{providerFilter&&!providers.data?.some(provider=>provider.id===providerFilter)?<option value={providerFilter}>所选来源待核对</option>:null}{providers.data?.map(provider=><option key={provider.id} value={provider.id}>{resourceName(provider.id,"upstream",provider.name)}</option>)}</select><span className="muted">{models.data&&filteredSourcesReady?visibleModels.length:"—"} / {models.data?.length??"—"} 个已接入模型</span></div>
      <ReadStatus pending={false} error={providers.error} hasData={!!providers.data} retry={()=>void providers.refetch()}/>
      <ReadStatus pending={false} error={topology.error} hasData={!!topology.data} retry={()=>void topology.refetch()}/>
      <div className="card tablewrap models-inventory">
        <table>
          <thead>
            <tr>
              <th>原始模型 ID / 来源</th><th>上游协议</th><th>开放状态</th><th>运行可用性</th><th>操作</th>
            </tr>
          </thead>
          <tbody>
            {(searchText||providerFilter)&&models.data&&filteredSourcesReady&&visibleModels.length===0?<tr><td colSpan={5} className="empty-state">没有匹配的模型。<button className="secondary" onClick={()=>{setSearchText("");setProviderFilter("");}}>清除筛选</button></td></tr>:null}
            {providerFilter&&!filteredSourcesReady?<tr><td colSpan={5} className="empty-state">{topology.isError?"来源读取失败，请重新读取后筛选。":"正在读取模型来源…"}</td></tr>:null}
            {(filteredSourcesReady?visibleModels:[]).map((model) => {
              const sources=connectionsFor(model.id);
              return <tr key={model.id}>
                <td data-label="模型 ID"><strong className="mono">{model.model_name}</strong>{model.display_name!==model.model_name?<span className="entity-meta">{model.display_name}</span>:null}<div className="model-source-preview">{topology.isError?"连接读取失败":!topology.data?"读取连接…":!sources.length?"未添加连接":[...new Map(sources.map(c=>[c.endpoint_id,c])).values()].map(c=>{const endpoint=topology.data?.endpoints.find(e=>e.id===c.endpoint_id);return <span key={c.id}>{resourceName(endpoint?.upstream_id??"","upstream",providers.data?.find(p=>p.id===endpoint?.upstream_id)?.name)}{sources.some(s=>s.endpoint_id===c.endpoint_id&&s.enabled)?"":" · 已停用"}</span>;})}</div></td>
                <td data-label="上游协议"><div className="model-source-preview">{topology.isError?"未读取":!topology.data?"读取中…":[...new Set(sources.map(source=>topology.data?.endpoints.find(endpoint=>endpoint.id===source.endpoint_id)?.api_format).filter((format):format is string=>!!format))].map(format=><span key={format}>{protocolName(format)}</span>)}</div>{topology.data&&!sources.length?"—":null}</td>
                <td data-label="开放状态"><StatusBadge status={model.status}>{model.status==="active"?"已启用":"已停用"}</StatusBadge></td>
                <td data-label="运行可用性">{context?.status!=="active"?<span className="entity-meta">应用后可核对运行准入</span>:topology.isError?<span>运行来源未确认</span>:!topology.data?<span>读取来源…</span>:topology.data.routes.filter(route=>route.public_model_id===model.id).length?<div className="model-source-preview">{topology.data.routes.filter(route=>route.public_model_id===model.id).map((route,index)=><Link key={route.id} to={`/runtime?${new URLSearchParams({route_id:route.id,requested_model:model.model_name})}`}>核对实时准入{topology.data!.routes.filter(candidate=>candidate.public_model_id===model.id).length>1?` · 路由 ${index+1}`:""}</Link>)}<span>按请求协议与运行条件判定</span></div>:<span>尚未连接路由</span>}</td>
                <td className="row-actions"><button className="secondary" onClick={()=>boundary.request(()=>setConnectionTarget(model))}>管理连接</button><button className="secondary" onClick={()=>boundary.request(()=>setInspected(model))}>详情</button>
                  <details className="row-menu"><summary>更多</summary><div>
                    <button className="secondary" onClick={()=>openModelEditor(model)}>编辑模型</button>
                    <button className="secondary" onClick={()=>openModelAliases(model)}>管理别名</button>
                    {editable?<button className="secondary" onClick={()=>openRouteCreate(model)}>配置路由</button>:null}
                    <button className="danger" onClick={()=>openModelDelete(model)}>删除模型</button>
                  </div></details>
                </td>
              </tr>;
            })}
          </tbody>
        </table>
        {models.data?.length === 0 ? (
          <div className="empty-state" data-kind="empty">
            <p>{t.state.empty}</p>
          </div>
        ) : null}
      </div>

      <details className="models-advanced" open={createdRouteId!==undefined?true:undefined}><summary onClick={event=>{if(candidateActive){event.preventDefault();boundary.request(()=>{});}}}>高级路由、候选与别名</summary>          <button
            type="button"
            disabled={!editable}
            title={editable ? undefined : t.version.readOnly}
            onClick={() => openModelEditor()}
          >
            高级模型配置
          </button><RouteWorkbench onCandidateActiveChange={setCandidateActive} focusRouteId={createdRouteId} editable={editable} modelSeed={modelSeed} /></details>

      {connectionTarget?<ModelConnectionsDialog model={connectionTarget} onClose={()=>setConnectionTarget(undefined)} onSaved={version=>{setConnectionTarget(undefined);useVersionStore.getState().select(version);invalidate();}} onAdd={()=>{const route=topology.data?.routes.find(r=>r.public_model_id===connectionTarget.id);const mappings=[...new Set(topology.data?.candidates.filter(c=>c.route_id===route?.id).map(c=>c.upstream_model)??[])];if(mappings.length>1){setNotice("该模型已有多种上游映射，请在高级路由配置中选择具体路径。");return;}setConnectionSeed({model:mappings[0]??connectionTarget.model_name,endpoint:"",targetModelId:connectionTarget.id});setConnectionTarget(undefined);setConnecting(true);}}/>:null}
      {inspected === undefined ? null : <ObjectInspector title={inspected.model_name} scope="已接入模型" onClose={() => setInspected(undefined)} facts={[
        ["模型名称", inspected.model_name], ["显示名称", inspected.display_name || inspected.model_name], ["开放状态", inspected.status==="active"?"已启用":"已停用"],
        ["声明能力", enabledCapabilities(inspected.capabilities).join(" · ") || "未声明"],
      ]}>
        <p className="small muted">这是版本配置中的公开模型；客户端实际可见性还取决于 serving 配置和访问授权。</p>
        <div className="sheet-actions"><button onClick={() => {openModelEditor(inspected);setInspected(undefined);}}>编辑模型</button></div>
      </ObjectInspector>}

      {modelEditor ? <ModelEditorDialog key={`${modelEditor.owner.selection}:${modelEditor.initial?.id??"new"}`} initial={modelEditor.initial} owner={modelEditor.owner} onClose={()=>setModelEditor(undefined)} onDone={finishModelEditor} onReview={receipt=>reviewModelReceipt(receipt,modelEditor.owner,()=>setModelEditor(undefined))}/> : null}

      {aliasTarget?<ModelAliasDialog key={`${aliasTarget.owner.selection}:${aliasTarget.model.id}`} model={aliasTarget.model} owner={aliasTarget.owner} onClose={()=>setAliasTarget(undefined)} onDone={receipt=>{setAliasTarget(undefined);settleModelReceipt(receipt);void queryClient.resetQueries({queryKey:["routing-inventory"]});}} onReview={receipt=>reviewModelReceipt(receipt,aliasTarget.owner,()=>setAliasTarget(undefined))}/>:null}

      {routeTarget?<RouteCreateDialog key={`${routeTarget.owner.selection}:${routeTarget.model.id}`} model={routeTarget.model} owner={routeTarget.owner} onClose={()=>setRouteTarget(undefined)} onDone={({receipt,routeId})=>{setRouteTarget(undefined);if(routeId)setCreatedRouteId(routeId);settleModelReceipt(receipt);if(routeId)setNotice("路由已保存；请在下方路由工作台添加候选后再校验和发布。");}} onReview={receipt=>reviewModelReceipt(receipt,routeTarget.owner,()=>setRouteTarget(undefined))}/>:null}

      {confirmDelete?<ModelDeleteDialog key={`${confirmDelete.owner.selection}:${confirmDelete.model.id}`} model={confirmDelete.model} owner={confirmDelete.owner} onClose={()=>setConfirmDelete(undefined)} onDone={receipt=>{setConfirmDelete(undefined);settleModelReceipt(receipt);}} onReview={receipt=>reviewModelReceipt(receipt,confirmDelete.owner,()=>setConfirmDelete(undefined))}/>:null}
    </section>
  );
}
