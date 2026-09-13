import { Link } from "react-router-dom";
import { useModelConnections } from "../models/useModelConnections";
import { protocolName } from "../accounts/presentation";
import { resourceName, referenceText } from "../../utils/resourceNames";
import { ReadStatus } from "../../components/ReadStatus";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useSearchParams } from "react-router-dom";
import { useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { ChipsInput } from "../../components/ChipsInput";
import { Sheet } from "../../components/Sheet";
import { ObjectInspector } from "../../components/ObjectInspector";
import { StatusBadge } from "../../components/StatusBadge";
import { useMessages } from "../../i18n/messages";
import { useVersionStore } from "../config-versions/versionStore";
import type { EgressPolicy } from "../egress/model";
import { SubresourcePanel } from "./SubresourcePanel";
import { ProviderDialog } from "./ProviderDialog";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import { ConfigurationTaskNotice } from "../config-versions/ConfigurationTaskNotice";
import type { PublicModel, RouteListItem, CandidateRecord, RoutingPage } from "../models/model";
import type { ManagedEndpoint, InventoryPage } from "../accounts/inventory";

type Upstream = Readonly<{
  id: string;
  name: string;
  kind: string;
  enabled: boolean;
  tags: readonly string[];
  egress_policy_id?: string | null;
}>;

function providerKindLabel(kind:string) {
  return ({"openai-compatible":"OpenAI 兼容","anthropic-compatible":"Anthropic 兼容",codex:"Codex / ChatGPT",claude:"Claude",kimi:"Kimi",kiro:"Kiro","grok.official":"Grok API","grok-web-native":"Grok Web","grok-console-native":"Grok Console","grok-build-native":"Grok Build"} as Record<string,string>)[kind]??kind;
}

const KIND_SUGGESTIONS = [
  "grok.official",
  "grok.build",
  "grok.web",
  "kiro",
  "openai-compatible",
  "anthropic-compatible",
];

type DraftUpstream = {
  original?: Upstream;
  id: string;
  name: string;
  kind: string;
  enabled: boolean;
  tags: string[];
  egress_policy_id: string;
  isNew: boolean;
};

function toDraft(upstream: Upstream): DraftUpstream {
  return {
    original:upstream,
    id: upstream.id,
    name: resourceName(upstream.id,"upstream",upstream.name),
    kind: upstream.kind,
    enabled: upstream.enabled,
    tags: [...upstream.tags],
    egress_policy_id: upstream.egress_policy_id ?? "",
    isNew: false,
  };
}

function toInput(draft: DraftUpstream) {
  return {
    id: draft.id,
    name: draft.name,
    kind: draft.kind,
    enabled: draft.enabled,
    tags: draft.tags,
    egress_policy_id: draft.egress_policy_id === "" ? null : draft.egress_policy_id,
  };
}

export function UpstreamsPage() {
  const t = useMessages();
  const queryClient = useQueryClient();
  const context = useVersionStore((s) => s.context);
  const editable = context?.status === "draft";
  const scope = context?.configVersionId;
  const [draft, setDraft] = useState<DraftUpstream | undefined>();
  const [inspected, setInspected] = useState<Upstream>();
  const [confirmDelete, setConfirmDelete] = useState<Upstream | undefined>();
  const [searchParams,setSearchParams] = useSearchParams();
  const [expanded, setExpanded] = useState<string | undefined>(searchParams.get("upstream_id") ?? undefined);
  const [actionError, setActionError] = useState<string | undefined>();
  const [adding,setAdding]=useState(searchParams.get("add")==="provider");
  const closeAdding=()=>{setAdding(false);const next=new URLSearchParams(searchParams);next.delete("add");setSearchParams(next,{replace:true});};
  const [workingId,setWorkingId]=useState<string>();
  const topology=useModelConnections();
  const [filter,setFilter]=useState("");
  const [kindFilter,setKindFilter]=useState("");

  const upstreams = useQuery({
    queryKey: ["upstreams", scope],
    queryFn: () => call<Upstream[]>("listUpstreams", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });
  const policies = useQuery({
    queryKey: ["egress", scope],
    queryFn: () => call<EgressPolicy[]>("listEgressPolicies", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });

  const invalidate = () => void queryClient.invalidateQueries({ queryKey: ["upstreams", scope] });

  const save = useMutation({
    mutationFn: async (input: DraftUpstream) => {
      const task=await beginConfigurationTask(`编辑提供商 · ${input.name}`);setWorkingId(task.version.id);
      if(input.original&&JSON.stringify(await task.read<Upstream>("getUpstream",{path:{upstream_id:input.id}}))!==JSON.stringify(input.original))throw new Error("提供商已被修改，请重新读取后编辑。");
      await task.mutate(input.isNew?"createUpstream":"updateUpstream",{...(input.isNew?{}:{path:{upstream_id:input.id}}),body:toInput(input)});
      return task.finish();
    },
    onSuccess: (version) => {
      setDraft(undefined);
      invalidate();
      useVersionStore.getState().select(version);
    },
    onError: (error) => setActionError(asAppError(error).message),
  });

  const remove = useMutation({
    mutationFn: async (id: string) => {
      const task=await beginConfigurationTask("移除提供商");setWorkingId(task.version.id);
      const load=async<T,>(operation:"listManagedEndpoints"|"listRoutes"|"listRouteCandidates",query:Record<string,string>={})=>{
        const rows:T[]=[];let cursor:string|undefined;let revision:string|undefined;
        do {
          const page=await task.read<RoutingPage<T>|InventoryPage<T>>(operation,{query:{...query,limit:100,...(cursor?{cursor}:{})}});
          if(revision&&revision!==page.revision)throw new Error("连接已变化，请重新核对提供商。");
          revision=page.revision;rows.push(...page.items);cursor=page.next_cursor??undefined;
          if(rows.length>=10000&&cursor)throw new Error("关联资源超出本次操作范围，请使用高级配置。");
        }while(cursor);
        return rows;
      };
      const endpoints=new Set((await load<ManagedEndpoint>("listManagedEndpoints",{upstream_id:id})).map((row)=>row.id));
      const candidates=await load<CandidateRecord>("listRouteCandidates");
      const routes=await load<RouteListItem>("listRoutes");
      const models=await task.read<PublicModel[]>("listPublicModels");
      const affected=new Set(candidates.filter((row)=>endpoints.has(row.endpoint_id)).map((row)=>row.route_id));
      const remaining=new Set(candidates.filter((row)=>!endpoints.has(row.endpoint_id)&&row.enabled).map((row)=>row.route_id));
      const paused=new Set(routes.filter((row)=>affected.has(row.id)&&!remaining.has(row.id)).map((row)=>row.public_model_id));
      await task.mutate("deleteUpstream",{path:{upstream_id:id}});
      for(const model of models)if(paused.has(model.id)&&model.status==="active")await task.mutate("updatePublicModel",{path:{public_model_id:model.id},body:{...model,status:"disabled"}});
      return task.finish();
    },
    onSuccess: (version) => {
      setConfirmDelete(undefined);
      invalidate();
      useVersionStore.getState().select(version);
    },
    onError: (error) => setActionError(asAppError(error).message),
  });

  function onSubmit(event: FormEvent) {
    event.preventDefault();
    if (draft !== undefined) {
      save.mutate(draft);
    }
  }

  return (
    <section>
      <header className="page-head">
        <h2>{t.nav.upstreams}</h2>
        <div className="page-actions">
          <button
            type="button"
            onClick={() => setAdding(true)}
          >
            添加提供商
          </button>
        </div>
      </header>

      {actionError !== undefined ? (
        <p role="alert" className="action-error">
          {actionError}
          <button type="button" onClick={() => setActionError(undefined)}>
            清除
          </button>
        </p>
      ) : null}

      <ReadStatus pending={!!scope&&upstreams.isPending} error={upstreams.error} hasData={upstreams.data !== undefined} retry={() => void upstreams.refetch()} />

      <div className="data-toolbar"><input type="search" aria-label="搜索提供商或模型" placeholder="搜索提供商、地址或模型 ID" value={filter} onChange={e=>setFilter(e.target.value)}/><select aria-label="渠道类型" value={kindFilter} onChange={e=>setKindFilter(e.target.value)}><option value="">全部渠道</option>{[...new Set(upstreams.data?.map(p=>p.kind)??[])].map(kind=><option key={kind} value={kind}>{providerKindLabel(kind)}</option>)}</select></div>
      <ReadStatus pending={false} error={topology.error} hasData={!!topology.data} retry={()=>void topology.refetch()}/>
      <div className={`provider-workspace${expanded?" has-detail":""}`}><div className="provider-list">
        {(upstreams.data??[]).filter(p=>!kindFilter||p.kind===kindFilter).map(upstream=>{
          const endpoints=topology.data?.endpoints.filter(e=>e.upstream_id===upstream.id)??[];
          const endpointIds=new Set(endpoints.map(e=>e.id));
          const models=[...new Set(topology.data?.candidates.filter(c=>endpointIds.has(c.endpoint_id)).map(c=>c.upstream_model)??[])];
          const name=resourceName(upstream.id,"upstream",upstream.name);
          if(![name,upstream.kind,...models,...endpoints.map(e=>e.base_url)].join(" ").toLowerCase().includes(filter.toLowerCase()))return null;
          return <article className="provider-card" key={upstream.id}>
            <header><div><span className="provider-kind">{providerKindLabel(upstream.kind)}</span><h3>{name}</h3></div><StatusBadge status={upstream.enabled?"active":"disabled"}>{upstream.enabled?"已启用":"已停用"}</StatusBadge></header>
            <div className="provider-connections">{!topology.data?"读取连接…":endpoints.length?endpoints.map(e=><span key={e.id}>{protocolName(e.api_format)} · {new URL(e.base_url).host}{e.enabled?"":" · 已停用"}</span>):"尚未添加接口"}</div>
            <div className="provider-models"><span className="muted">已开放模型 <strong>{topology.data?models.length:"—"}</strong></span><Link to={`/catalog?upstream_id=${encodeURIComponent(upstream.id)}`}>上游目录</Link><Link to={`/models?add=model${endpoints[0]?`&from_endpoint=${encodeURIComponent(endpoints[0].id)}`:""}`}>开放模型</Link></div>
            <footer><div className="row-actions">
              <button className="secondary" onClick={()=>setExpanded(expanded===upstream.id?undefined:upstream.id)}>{expanded===upstream.id?"收起接口":"接口与账号"}</button>
              <button className="secondary" onClick={()=>{save.reset();setWorkingId(undefined);setActionError(undefined);setDraft(toDraft(upstream));}}>编辑</button>
              <details className="row-menu"><summary>更多</summary><div><button className="secondary" onClick={()=>setInspected(upstream)}>详情</button><button className="danger" onClick={()=>{remove.reset();setWorkingId(undefined);setActionError(undefined);setConfirmDelete(upstream);}}>移除提供商</button></div></details>
            </div></footer>
          </article>;
        })}
        {!scope||upstreams.data?.length===0?<div className="empty-state">添加提供商，设置接口地址并连接账号。</div>:null}
      </div>

      {expanded?<aside className="provider-detail" aria-label="提供商接口与账号"><header><h3>{resourceName(expanded,"upstream",upstreams.data?.find(row=>row.id===expanded)?.name)}</h3><button className="secondary" onClick={()=>setExpanded(undefined)}>关闭</button></header><SubresourcePanel upstreamId={expanded}/></aside>:null}
      </div>

      {adding?<ProviderDialog onClose={closeAdding} onSaved={(version)=>{closeAdding();invalidate();useVersionStore.getState().select(version);}}/>:null}

      {inspected === undefined ? null : <ObjectInspector title={resourceName(inspected.id, "upstream", inspected.name)} scope={`配置版本 ${resourceName(scope ?? "—", "config")}`} onClose={() => setInspected(undefined)} facts={[
        ["上游 ID", inspected.id], ["Provider 家族", inspected.kind], ["配置启用", inspected.enabled ? "已启用" : "已停用"],
        ["出口策略", inspected.egress_policy_id], ["标签", inspected.tags.join(" · ") || "—"],
      ]}>
        <p className="small muted">配置启用不代表实时认证、quota 或调度可用。</p>
        <div className="sheet-actions"><button className="secondary" onClick={() => { setExpanded(inspected.id); setInspected(undefined); }}>查看端点与凭据</button>
          <button onClick={() => { setDraft(toDraft(inspected)); setInspected(undefined); }}>编辑提供商</button></div>
      </ObjectInspector>}

      {draft !== undefined ? (
        <Sheet title={draft.isNew ? "新建上游" : `编辑 ${resourceName(draft.id,"upstream",draft.name)}`} onEscape={() => !save.isPending&&setDraft(undefined)}>
          <form className="sheet-form" onSubmit={onSubmit}>
            <ConfigurationTaskNotice workingId={workingId} error={save.error} onReview={(version)=>{setDraft(undefined);useVersionStore.getState().select(version);}}/>
            {draft.isNew ? (
              <label>
                上游 ID(创建后不可变)
                <input
                  className="mono"
                  required
                  maxLength={128}
                  value={draft.id}
                  onChange={(event) => setDraft({ ...draft, id: event.target.value })}
                />
              </label>
            ) : null}
            <label>
              名称
              <input
                required
                maxLength={256}
                value={draft.name}
                onChange={(event) => setDraft({ ...draft, name: event.target.value })}
              />
            </label>
            <label>
              渠道类型
              <input
                className="mono"
                required
                maxLength={128}
                list="kind-suggestions"
                value={draft.kind}
                onChange={(event) => setDraft({ ...draft, kind: event.target.value })}
              />
              <datalist id="kind-suggestions">
                {KIND_SUGGESTIONS.map((kind) => (
                  <option key={kind} value={kind} />
                ))}
              </datalist>
            </label>
            <label className="toggle-row">
              <input
                type="checkbox"
                checked={draft.enabled}
                onChange={(event) => setDraft({ ...draft, enabled: event.target.checked })}
              />
              启用
            </label>
            <label>
              标签
              <ChipsInput
                value={draft.tags}
                onChange={(tags) => setDraft({ ...draft, tags })}
                formatLabel={referenceText}
                placeholder="回车添加"
              />
            </label>
            <label>
              出口策略
              <select
                value={draft.egress_policy_id}
                onChange={(event) => setDraft({ ...draft, egress_policy_id: event.target.value })}
              >
                <option value="">(无)</option>
                {(policies.data ?? []).map((policy) => (
                  <option key={policy.id} value={policy.id}>
                    {resourceName(policy.id, "policy", policy.name)}
                  </option>
                ))}
              </select>
            </label>
            <div className="sheet-actions">
              <button type="button" className="secondary" disabled={save.isPending} onClick={() => setDraft(undefined)}>
                取消
              </button>
              <button type="submit" disabled={save.isPending}>
                {editable?"保存到草稿":"保存并应用"}
              </button>
            </div>
          </form>
        </Sheet>
      ) : null}

      {confirmDelete !== undefined ? (
        <Sheet title="移除提供商" onEscape={() => !remove.isPending&&setConfirmDelete(undefined)}>
          <ConfigurationTaskNotice workingId={workingId} error={remove.error} onReview={(version)=>{setConfirmDelete(undefined);useVersionStore.getState().select(version);}}/>
          <p className="reveal-warning">
            移除 <strong>{resourceName(confirmDelete.id,"upstream",confirmDelete.name)}</strong>
            {confirmDelete.kind.endsWith("-native")?" 及其接口和候选连接；Grok 渠道账号池保留。":" 及其全部账号、接口和候选连接。"}
            不再有启用候选的关联模型会同时停用，历史请求和费用保留。
          </p>
          <div className="sheet-actions">
            <button type="button" className="secondary" disabled={remove.isPending} onClick={() => setConfirmDelete(undefined)}>
              取消
            </button>
            <button
              type="button"
              className="danger"
              disabled={remove.isPending}
              onClick={() => remove.mutate(confirmDelete.id)}
            >
              确认删除
            </button>
          </div>
        </Sheet>
      ) : null}
    </section>
  );
}
