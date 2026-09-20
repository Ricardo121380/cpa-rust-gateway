import { ResourcePicker } from "../../components/ResourcePicker";
import { resourceName } from "../../utils/resourceNames";
import { ResourceIdentity } from "../../components/ResourceIdentity";
import { RoutingInventory } from "./RoutingInventory";
import { CandidateDialog, type CandidateAction } from "./CandidateDialog";
import { RouteDialog, type RouteAction } from "./RouteDialog";
import { captureDraftRoutingOwner, isModelActionOwner, type DraftRoutingOwner } from "./advancedRoutingTask";
import { sameRoute } from "./advancedRoutingModel";
import { sameCandidate } from "./modelTask";
import { useModelConnections } from "./useModelConnections";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Link, useNavigate } from "react-router-dom";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { ObjectInspector } from "../../components/ObjectInspector";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import {
  type CandidateRecord,
  ROUTE_POLICY,
  routeErrorLabel,
  type RouteRecord,
  type RouteValidation,
} from "./model";

export function RouteWorkbench({
  focusRouteId,
  editable,
  modelSeed,
}: Readonly<{
  focusRouteId: string | undefined;
  editable: boolean;
  modelSeed?: { model: string; endpoint: string } | undefined;
}>) {
  const queryClient = useQueryClient();
  const navigate=useNavigate();
  const context = useVersionStore((s) => s.context);
  const scope = context?.configVersionId;

  const [field, setField] = useState("");
  const [loaded, setLoaded] = useState<string | undefined>();
  const [candidateAction,setCandidateAction]=useState<CandidateAction>();
  const [routeAction,setRouteAction]=useState<RouteAction>();
  const [inspecting, setInspecting] = useState(false);
  const [notice, setNotice] = useState<string | undefined>();
  const [error, setError] = useState<string | undefined>();
  const [evidence,setEvidence]=useState<{routeId:string;revision:string;result:RouteValidation}>();
  const loadedRef=useRef(loaded);
  loadedRef.current=loaded;
  useEffect(()=>{setEvidence(undefined);},[loaded,context?.revision]);
  const topology=useModelConnections();

  const openCandidate=(kind:"edit"|"delete",candidate:CandidateRecord)=>{
    try{
      const route=topology.data?.routes.find(row=>row.id===candidate.route_id);
      const observed=topology.data?.candidates.find(row=>row.id===candidate.id&&row.route_id===candidate.route_id);
      if(!route||!sameCandidate(observed,candidate))throw new Error("连接清单已变化，请重新读取后操作。");
      setError(undefined);setCandidateAction({kind,owner:captureDraftRoutingOwner(),route,candidate});
    }catch(cause){setError(asAppError(cause).message);}
  };

  // The route ModelsPage just created is the one you almost certainly want, and
  // keep the direct handoff alongside the complete inventory.
  const pending =
    focusRouteId !== undefined && focusRouteId !== loaded
      ? focusRouteId
      : undefined;

  const route = useQuery({
    queryKey: ["route", scope, context?.revision, loaded],
    queryFn: () =>
      call<RouteRecord>(
        "getRoute",
        { path: { route_id: loaded ?? "" } },
        { versionScoped: true },
      ),
    enabled: scope !== undefined && loaded !== undefined,
    retry: false,
  });

  const validation = useMutation({
    mutationFn: async(input:Readonly<{routeId:string;owner:DraftRoutingOwner}>)=>{
      if(!isModelActionOwner(input.owner)||useVersionStore.getState().context?.revision!==input.owner.revision)throw new Error("草稿已变化，请重新校验。");
      const before=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:input.owner.id}});
      if(before.revision!==input.owner.revision)throw new Error("草稿已变化，请重新校验。");
      const result=await call<RouteValidation>("validateRoute",{path:{route_id:input.routeId},headers:{"X-Config-Version":input.owner.id}});
      const after=await call<ConfigVersionSummary>("getConfigVersion",{path:{config_version_id:input.owner.id}});
      if(!isModelActionOwner(input.owner)||after.revision!==input.owner.revision||useVersionStore.getState().context?.revision!==input.owner.revision)throw new Error("校验期间草稿已变化，请重新校验。");
      return result;
    },
    onSuccess:(result,input)=>{if(isModelActionOwner(input.owner)&&useVersionStore.getState().context?.revision===input.owner.revision&&loadedRef.current===input.routeId)setEvidence({routeId:input.routeId,revision:input.owner.revision,result});},
    onError:(cause,input)=>{if(isModelActionOwner(input.owner)&&loadedRef.current===input.routeId)setError(`校验未完成：${asAppError(cause).message}`);},
  });
  const startValidation=(routeId:string)=>{
    try{const owner=captureDraftRoutingOwner();loadedRef.current=routeId;setEvidence(undefined);setError(undefined);validation.mutate({routeId,owner});}
    catch(cause){setError(asAppError(cause).message);}
  };

  function onLoad(event: FormEvent) {
    event.preventDefault();
    const next = field.trim();
    if (next === "") {
      return;
    }
    validation.reset();
    setError(undefined);
    loadedRef.current=next;
    setLoaded(next);
  }

  const record = route.data;
  const visibleValidation=evidence&&evidence.routeId===record?.id&&evidence.revision===context?.revision?evidence.result:undefined;
  const openRoute=(kind:"edit"|"delete")=>{
    try{
      const baseline=topology.data?.routes.find(row=>row.id===record?.id);
      if(!record||!sameRoute(baseline,record))throw new Error("路由清单已变化，请重新读取后操作。");
      if(baseline?.policy!==ROUTE_POLICY)throw new Error("此历史调度策略只支持查看，不能改写。");
      setError(undefined);setRouteAction({kind,owner:captureDraftRoutingOwner(),route:baseline});
    }catch(cause){setError(asAppError(cause).message);}
  };

  return (
    <div className="card route-workbench" data-gap="top">
      <header className="page-head">
        <h3>路由工作台</h3>
      </header>

      <RoutingInventory
        editable={editable}
        onEdit={(candidate) => openCandidate("edit",candidate)}
        onDelete={(candidate) => openCandidate("delete",candidate)}
        onOpen={(id) => {
          setField(id);
          loadedRef.current=id;
          setLoaded(id);
          validation.reset();
        }}
      />

      {pending !== undefined ? (
        <p className="action-notice">
          刚创建了路由 <span>{resourceName(pending,"route")}</span>。
          <button
            type="button"
            onClick={() => {
              setField(pending);
              validation.reset();
              loadedRef.current=pending;
              setLoaded(pending);
            }}
          >
            打开它
          </button>
        </p>
      ) : null}

      {notice !== undefined ? (
        <p className="action-notice">
          {notice}
          <button type="button" onClick={() => setNotice(undefined)}>
            知道了
          </button>
        </p>
      ) : null}
      {error !== undefined ? (
        <p role="alert" className="action-error">
          {error}
          <button type="button" onClick={() => setError(undefined)}>
            清除
          </button>
        </p>
      ) : null}

      <form className="rw-load" onSubmit={onLoad}>
        <label>
          路由
          <ResourcePicker kind="route" required value={field} onChange={setField} />
        </label>
        <button type="submit" disabled={route.isFetching}>
          {route.isFetching ? "载入中…" : "载入"}
        </button>
      </form>

      {loaded !== undefined && route.isError ? (
        <div className="empty-state" data-kind="error">
          <p>{asAppError(route.error).message}</p>
        </div>
      ) : null}

      {record !== undefined ? (
        <>
          <table>
            <tbody>
              <tr>
                <th scope="row">所属公开模型</th>
                <td><ResourceIdentity id={record.public_model_id} kind="resource" /></td>
              </tr>
              <tr>
                <th scope="row">调度策略</th>
                <td className="mono">{record.policy}</td>
              </tr>
              <tr>
                <th scope="row">max_attempts</th>
                <td className="mono">{record.max_attempts}</td>
              </tr>
              <tr>
                <th scope="row">bootstrap_timeout_ms</th>
                <td className="mono">{record.bootstrap_timeout_ms}</td>
              </tr>
            </tbody>
          </table>

          <div className="rw-actions">
            <button className="secondary" onClick={() => setInspecting(true)}>
              路由详情
            </button>
            <button
              type="button"
              disabled={!editable}
              title={editable ? undefined : "仅草稿版本可编辑"}
              onClick={() => {
                try{
                  const route=topology.data?.routes.find(row=>row.id===record.id);
                  if(!route)throw new Error("路由清单尚未读取完成，请重新读取后添加来源。");
                  setError(undefined);setCandidateAction({kind:"add",owner:captureDraftRoutingOwner(),route,seed:modelSeed});
                }catch(cause){setError(asAppError(cause).message);}
              }}
            >
              加候选
            </button>
            <button
              type="button"
              className="secondary"
              disabled={validation.isPending||!editable}
              onClick={() => startValidation(record.id)}
            >
              {validation.isPending ? "校验中…" : "校验"}
            </button>
            <button
              type="button"
              className="secondary"
              disabled={!editable}
              title={editable ? undefined : "仅草稿版本可编辑"}
              onClick={() => openRoute("edit")}
            >
              编辑路由
            </button>
            <Link
              className="rw-link"
              to={`/runtime?route_id=${encodeURIComponent(record.id)}`}
            >
              去 Explain 看候选
            </Link>
            <button
              type="button"
              className="danger"
              disabled={!editable}
              title={editable ? undefined : "仅草稿版本可编辑"}
              onClick={() => openRoute("delete")}
            >
              删除路由
            </button>
          </div>

          <p className="stat-sub">
            配置资源列表显示候选定义；Explain 用于检查指定请求的选择结果。
          </p>

          {visibleValidation !== undefined ? (
            <div
              className="rw-validation"
              data-valid={visibleValidation.valid ? "true" : "false"}
            >
              <p>
                {visibleValidation.valid
                  ? "草稿拓扑校验通过"
                  : `草稿拓扑校验未通过 · ${visibleValidation.error_codes?.length ?? 0} 项`}
              </p>
              {visibleValidation.valid ? null : (
                <ul className="rw-codes">
                  {(visibleValidation.error_codes ?? []).map((code) => {
                    const label = routeErrorLabel(code);
                    return (
                      <li key={code}>
                        <span className="mono">{code}</span>
                        {label === undefined ? null : (
                          <span className="rw-code-help">{label}</span>
                        )}
                      </li>
                    );
                  })}
                </ul>
              )}
              <p className="stat-sub">
                此处只校验<strong>草稿拓扑</strong>
                (候选是否存在、端点是否在且启用、是否有 active
                凭据绑定)。能力准入与完整编译在<strong>发布</strong>时才发生 ——
                这里通过不等于发布会通过。
              </p>
            </div>
          ) : null}
        </>
      ) : null}

      {candidateAction?<CandidateDialog key={`${candidateAction.owner.selection}:${candidateAction.kind}:${candidateAction.candidate?.id??"new"}`} action={candidateAction} onClose={()=>setCandidateAction(undefined)} onDone={(receipt,routeId)=>{const kind=candidateAction.kind;setCandidateAction(undefined);if(receipt.kind==="unconfirmed"){navigate("/versions");return;}void queryClient.resetQueries({queryKey:["routing-inventory",scope]});loadedRef.current=routeId;setLoaded(routeId);validation.reset();startValidation(routeId);setNotice(kind==="delete"?"候选已从草稿删除；路由和授权保留。":"候选已保存到草稿；正在重新校验路由。");}}/>:null}

      {inspecting && record !== undefined ? (
        <ObjectInspector
          title={resourceName(record.id,"route")}
          scope={`配置版本 ${resourceName(scope ?? "—", "config")} · 路由`}
          onClose={() => setInspecting(false)}
          facts={[
            ["公开模型 ID", record.public_model_id],
            ["调度策略", record.policy],
            ["最大尝试次数", record.max_attempts],
            ["启动超时 (ms)", record.bootstrap_timeout_ms],
          ]}
        >
          <div className="sheet-actions">
            <button
              disabled={!editable}
              onClick={() => {
                setInspecting(false);
                openRoute("edit");
              }}
            >
              编辑路由
            </button>
          </div>
        </ObjectInspector>
      ) : null}

      {routeAction?<RouteDialog key={`${routeAction.owner.selection}:${routeAction.kind}:${routeAction.route.id}`} action={routeAction} onClose={()=>setRouteAction(undefined)} onDone={(receipt,doneAction)=>{setRouteAction(undefined);if(receipt.kind==="unconfirmed"){navigate("/versions");return;}void queryClient.resetQueries({queryKey:["routing-inventory",scope]});validation.reset();if(doneAction.kind==="delete"){setLoaded(undefined);setNotice("路由已从草稿删除；候选及访问组授权随之移除，公开模型保留。");}else{void queryClient.resetQueries({queryKey:["route",scope]});setNotice("路由参数已保存到草稿；请重新校验后发布。");}}}/> : null}
    </div>
  );
}
