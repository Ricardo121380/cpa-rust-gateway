import { ResourceIdentity } from "../../components/ResourceIdentity";
import { resourceName } from "../../utils/resourceNames";
import { ReadStatus } from "../../components/ReadStatus";
// Egress policies: allowlist-based SSRF boundary (docs/07 §7.7).
// PATCH is full-replacement (C11) — the edit sheet always loads and submits
// the complete EgressPolicyInput.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, useRef, useEffect, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { ChipsInput } from "../../components/ChipsInput";
import { Sheet, SheetDismissButton } from "../../components/Sheet";
import { InlineWorkspace } from "../../components/InlineWorkspace";
import { useOperationBoundary } from "../../components/OperationBoundary";
import { beginConfigurationTask } from "../config-versions/configurationTask";
import { ConfigurationTaskNotice } from "../config-versions/ConfigurationTaskNotice";
import { useSessionStore } from "../../session/sessionStore";
import { ObjectInspector } from "../../components/ObjectInspector";
import { useMessages } from "../../i18n/messages";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import { CompatibleProxyPanel } from "./CompatibleProxyPanel";
import { ProviderEgressCard } from "../runtime/RuntimePage";
import { useNowTick } from "../../utils/useNowTick";
import "./compatible.css";
import {
  normalizedMaxRedirects,
  referencingUpstreams,
  validateHostEntry,
  validatePortEntry,
  type EgressPolicy,
} from "./model";

type UpstreamSummary = Readonly<{ id: string; egress_policy_id?: string | null }>;

type DraftPolicy = {
  original?:EgressPolicy;
  source?:{id:string;revision:string};
  id: string;
  name: string;
  hosts: string[];
  ports: string[];
  cidrs: string[];
  redirect_mode: "deny" | "revalidate";
  max_redirects: number;
  isNew: boolean;
};

function emptyDraft(): DraftPolicy {
  return {
    id: "",
    name: "",
    hosts: [],
    ports: ["443"],
    cidrs: [],
    redirect_mode: "deny",
    max_redirects: 0,
    isNew: true,
  };
}

function toDraft(policy: EgressPolicy): DraftPolicy {
  return {
    original:policy,
    id: policy.id,
    name: policy.name,
    hosts: [...policy.allowed_hosts],
    ports: policy.allowed_ports.map(String),
    cidrs: [...policy.allowed_cidrs],
    redirect_mode: policy.redirect_mode,
    max_redirects: policy.max_redirects,
    isNew: false,
  };
}

function toInput(draft: DraftPolicy) {
  return {
    id: draft.id,
    name: draft.name,
    allowed_schemes: draft.original?.allowed_schemes ?? ["https"],
    allowed_hosts: draft.hosts,
    allowed_ports: draft.ports.map(Number),
    allowed_cidrs: draft.cidrs,
    redirect_mode: draft.redirect_mode,
    max_redirects: normalizedMaxRedirects(draft.redirect_mode, draft.max_redirects),
  };
}

export function EgressPage() {
  const admission=useOperationBoundary();
  const submitted=useRef(false),live=useRef(true);
  const [owner]=useState(()=>({session:useSessionStore.getState().generation,selection:useVersionStore.getState().selectionGeneration}));
  useEffect(()=>{live.current=true;return()=>{live.current=false;};},[]);
  const owned=()=>live.current&&owner.session===useSessionStore.getState().generation&&owner.selection===useVersionStore.getState().selectionGeneration;
  const [workingId,setWorkingId]=useState<string>(),[receipt,setReceipt]=useState<ConfigVersionSummary>();
  const [deleteSource,setDeleteSource]=useState<{id:string;revision:string}>();
  const nowMs = useNowTick(60_000);
  const t = useMessages();
  const queryClient = useQueryClient();
  const context = useVersionStore((s) => s.context);
  const editable = context?.status === "draft";
  const scope = context?.configVersionId;
  const [draft, setDraft] = useState<DraftPolicy | undefined>();
  // Native fields are observed by Sheet's input/change capture. Chips are
  // button-driven, however, so they need an explicit dirty signal as well.
  const [draftDirty, setDraftDirty] = useState(false);
  const [inspected, setInspected] = useState<EgressPolicy>();
  const [confirmDelete, setConfirmDelete] = useState<EgressPolicy | undefined>();
  const [actionError, setActionError] = useState<string | undefined>();

  const policies = useQuery({
    queryKey: ["egress", scope],
    queryFn: () => call<EgressPolicy[]>("listEgressPolicies", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });
  const upstreams = useQuery({
    queryKey: ["upstreams", scope],
    queryFn: () => call<UpstreamSummary[]>("listUpstreams", {}, { versionScoped: true }),
    enabled: scope !== undefined,
  });

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ["egress", scope] });
    void queryClient.invalidateQueries({ queryKey: ["upstreams", scope] });
  };

  const save = useMutation({
    mutationFn: async (input:DraftPolicy)=>{
      const task=await beginConfigurationTask("维护出口策略",input.source,"deferred");setWorkingId(task.version.id);useVersionStore.getState().rememberPending(task.version);
      if(input.original){const current=(await task.read<EgressPolicy[]>("listEgressPolicies")).find(row=>row.id===input.id);if(!current||JSON.stringify(toInput(toDraft(current)))!==JSON.stringify(toInput(toDraft(input.original))))throw new Error("策略已变化，请重新打开后核对。");}
      await task.mutate(input.isNew?"createEgressPolicy":"updateEgressPolicy",{...(input.isNew?{}:{path:{egress_policy_id:input.id}}),body:toInput(input)});
      return task.finish();
    },
    onSuccess:version=>{if(owned()){useVersionStore.getState().rememberPending(version);useVersionStore.getState().advanceFromEtag(version.revision);setReceipt(version);invalidate();}},
    onError:error=>{if(owned())setActionError(asAppError(error).message);},
  });
  const remove=useMutation({mutationFn:async(id:string)=>{
    const task=await beginConfigurationTask("删除出口策略",deleteSource,"deferred");setWorkingId(task.version.id);useVersionStore.getState().rememberPending(task.version);
    const current=(await task.read<EgressPolicy[]>("listEgressPolicies")).find(row=>row.id===id);
    if(!current||!confirmDelete||JSON.stringify(toInput(toDraft(current)))!==JSON.stringify(toInput(toDraft(confirmDelete))))throw new Error("策略已变化，请重新核对删除目标。");
    await task.mutate("deleteEgressPolicy",{path:{egress_policy_id:id}});return task.finish();
  },onSuccess:version=>{if(owned()){useVersionStore.getState().rememberPending(version);useVersionStore.getState().advanceFromEtag(version.revision);setReceipt(version);invalidate();}},onError:error=>{if(owned())setActionError(asAppError(error).message);}});
  const close=()=>{setDraft(undefined);setConfirmDelete(undefined);setReceipt(undefined);};
  const review=(version:ConfigVersionSummary)=>{if(owned()){useVersionStore.getState().rememberPending(version);useVersionStore.getState().select(version);}};

  function onSubmit(event: FormEvent) {
    event.preventDefault();
    if (draft !== undefined&&!submitted.current) {
      submitted.current=true;save.mutate(draft);
    }
  }

  function beginDraft(next: DraftPolicy) {
    admission.request(()=>{submitted.current=false;setReceipt(undefined);setWorkingId(undefined);setActionError(undefined);save.reset();
    setDraftDirty(false);
    setDraft({...next,source:context?{id:context.configVersionId,revision:context.revision}:undefined});});
  }

  if (scope === undefined) {
    return (
      <section>
        <h2>{t.nav.egress}</h2>
        <div className="card empty-state" data-kind="empty">
          <p>请到“配置版本”发布或选择一份配置。</p>
        </div>
      </section>
    );
  }

  return (
    <section>
      <header className="page-head">
        <h2>{t.nav.egress}</h2>
        <div className="page-actions">
          <button
            type="button"
            disabled={!editable}
            title={editable ? undefined : t.version.readOnly}
            onClick={() => beginDraft(emptyDraft())}
          >
            新建出口策略
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

      {draft !== undefined ? (
        <InlineWorkspace
          title={receipt?"出口策略修改结果":draft.isNew ? "新建出口策略" : `编辑 ${resourceName(draft.id,"policy",draft.name)}`}
          description="限制上游允许访问的目标；变更会在当前草稿版本中保存。"
          onClose={close}
          busy={save.isPending}
          dirty={!receipt&&draftDirty}
          footer={receipt?<button onClick={close}>完成</button>:<><button type="button" className="secondary" disabled={save.isPending} onClick={()=>admission.request(close)}>取消</button><button type="submit" form="egress-policy-form" disabled={submitted.current||draft.hosts.length===0}>保存</button></>}
        >
          {receipt?<p role="status">策略已保存到草稿，尚未应用。</p>:<><ConfigurationTaskNotice workingId={workingId} error={save.error} onReview={review}/><form id="egress-policy-form" className="sheet-form" onChange={()=>setDraftDirty(true)} onSubmit={onSubmit}><fieldset disabled={submitted.current}>
            {draft.isNew ? (
              <label>
                策略 ID
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
              允许主机(精确域名,回车添加)
              <ChipsInput
                value={draft.hosts}
                onChange={(hosts) => { setDraftDirty(true); setDraft({ ...draft, hosts }); }}
                placeholder="relay.example.com"
                validate={validateHostEntry}
              />
            </label>
            <label>
              允许端口
              <ChipsInput
                value={draft.ports}
                onChange={(ports) => { setDraftDirty(true); setDraft({ ...draft, ports }); }}
                placeholder="443"
                validate={validatePortEntry}
              />
            </label>
            <label>
              允许 CIDR(可空)
              <ChipsInput
                value={draft.cidrs}
                onChange={(cidrs) => { setDraftDirty(true); setDraft({ ...draft, cidrs }); }}
                placeholder="203.0.113.0/24"
              />
            </label>
            <label>
              重定向模式
              <select
                value={draft.redirect_mode}
                onChange={(event) => {
                  const mode = event.target.value as DraftPolicy["redirect_mode"];
                  setDraft({
                    ...draft,
                    redirect_mode: mode,
                    max_redirects: normalizedMaxRedirects(mode, draft.max_redirects || 1),
                  });
                }}
              >
                <option value="deny">deny(拒绝一切重定向)</option>
                <option value="revalidate">revalidate(重定向目标重新过策略)</option>
              </select>
            </label>
            <label>
              最大重定向次数{draft.redirect_mode === "deny" ? "(deny 模式锁定为 0)" : "(1-5)"}
              <input
                type="number"
                min={draft.redirect_mode === "deny" ? 0 : 1}
                max={draft.redirect_mode === "deny" ? 0 : 5}
                disabled={draft.redirect_mode === "deny"}
                value={normalizedMaxRedirects(draft.redirect_mode, draft.max_redirects)}
                onChange={(event) =>
                  setDraft({ ...draft, max_redirects: Number(event.target.value) })
                }
              />
            </label>
            </fieldset>
          </form></>}
        </InlineWorkspace>
      ) : null}

      <ReadStatus pending={policies.isPending} error={policies.error} hasData={policies.data !== undefined} retry={() => void policies.refetch()} />

      <div className="card tablewrap">
        <table className="responsive-table">
          <thead>
            <tr>
              <th>名称</th>
              <th>主机</th>
              <th>端口</th>
              <th>重定向</th>
              <th>被引用</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            {(policies.data ?? []).map((policy) => {
              const refs = referencingUpstreams(policy.id, upstreams.data ?? []);
              return (
                <tr key={policy.id}>
                  <td data-label="名称"><ResourceIdentity id={policy.id} name={policy.name} kind="policy" /></td>
                  <td data-label="主机" className="mono">{policy.allowed_hosts.length} 条</td>
                  <td data-label="端口" className="mono">{policy.allowed_ports.join(", ")}</td>
                  <td data-label="重定向" className="mono">
                    {policy.redirect_mode}
                    {policy.redirect_mode === "revalidate" ? ` ≤${policy.max_redirects}` : ""}
                  </td>
                  <td data-label="被引用" className="mono">{refs.length > 0 ? refs.map((id) => resourceName(id, "upstream")).join(", ") : "—"}</td>
                  <td data-label="操作" className="row-actions">
                    <button className="secondary" onClick={() => admission.request(()=>setInspected(policy))}>详情</button>
                    <button
                      type="button"
                      className="secondary"
                      disabled={!editable}
                      onClick={() => beginDraft(toDraft(policy))}
                    >
                      编辑
                    </button>
                    <button
                      type="button"
                      className="danger"
                      disabled={!editable}
                      onClick={() => admission.request(()=>{submitted.current=false;setReceipt(undefined);setWorkingId(undefined);remove.reset();setDeleteSource(context?{id:context.configVersionId,revision:context.revision}:undefined);setConfirmDelete(policy);})}
                    >
                      删除
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
        {policies.data?.length === 0 ? (
          <div className="empty-state" data-kind="empty">
            <p>{t.state.empty}</p>
          </div>
        ) : null}
      </div>

      <CompatibleProxyPanel upstreams={upstreams.data ?? []} />
      <ProviderEgressCard scope={scope} nowMs={nowMs} />

      {inspected === undefined ? null : <ObjectInspector title={resourceName(inspected.id, "policy", inspected.name)} scope={`配置版本 ${resourceName(scope ?? "—", "config")} · 出口策略`} onClose={() => setInspected(undefined)} facts={[
        ["策略 ID", inspected.id], ["允许协议", inspected.allowed_schemes.join(" · ")],
        ["精确主机", inspected.allowed_hosts.join(" · ") || "无"], ["端口", inspected.allowed_ports.join(" · ") || "无"],
        ["CIDR", inspected.allowed_cidrs.join(" · ") || "无"], ["重定向模式", inspected.redirect_mode],
        ["重定向上限", inspected.max_redirects], ["引用上游", referencingUpstreams(inspected.id, upstreams.data ?? []).map(id=>resourceName(id,"upstream")).join(" · ") || "无"],
      ]}><div className="sheet-actions"><button disabled={!editable} onClick={() => { beginDraft(toDraft(inspected)); setInspected(undefined); }}>编辑策略</button></div></ObjectInspector>}

      {confirmDelete !== undefined ? (
        <Sheet title={receipt?"出口策略删除结果":"删除出口策略"} description="此操作会解除关联上游的出口策略；不会删除上游本身。" layout="confirm" tone="danger" onEscape={() => setConfirmDelete(undefined)} busy={remove.isPending} footer={receipt?<SheetDismissButton onDismiss={close}>完成</SheetDismissButton>:<><SheetDismissButton className="secondary" disabled={remove.isPending}>取消</SheetDismissButton><button type="button" className="danger" disabled={submitted.current} onClick={()=>{if(!submitted.current){submitted.current=true;remove.mutate(confirmDelete.id);}}}>确认删除</button></>}>
          {receipt?<p role="status">删除已保存到草稿，尚未应用。</p>:null}
          <ConfigurationTaskNotice workingId={workingId} error={remove.error} onReview={review}/>
          <p>
            删除 <span className="mono">{resourceName(confirmDelete.id,"policy",confirmDelete.name)}</span> 后,引用它的上游的
            egress_policy_id 将被清空(不会级联删除上游)。
          </p>
          {referencingUpstreams(confirmDelete.id, upstreams.data ?? []).length > 0 ? (
            <p className="reveal-warning">
              当前被引用:{referencingUpstreams(confirmDelete.id, upstreams.data ?? []).map(id=>resourceName(id,"upstream")).join("、")}
            </p>
          ) : null}
        </Sheet>
      ) : null}
    </section>
  );
}
