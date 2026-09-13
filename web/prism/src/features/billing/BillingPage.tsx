import {beginConfigurationTask} from "../config-versions/configurationTask";
import {ConfigurationTaskNotice} from "../config-versions/ConfigurationTaskNotice";
import { resourceName } from "../../utils/resourceNames";
import { ResourceIdentity } from "../../components/ResourceIdentity";
import { ProcessingStatus } from "./ProcessingStatus";
// 计费与价格目录 — the control surface for P13-05C / P13-07D.
//
// Two things live here and they are NOT the same scope, which is the single
// most important thing this page has to communicate:
//
//   PRICE CATALOGS ARE GLOBAL. Importing one while a draft is selected is not
//   scoped to that draft — the service lists them with no version filter at
//   all. Every config version sees a new catalog immediately.
//
//   THE POLICY BINDING IS PER CONFIG VERSION. Which catalog routing compares
//   against, and whether it compares at all, belongs to the selected draft.
//
// See features/billing/model.ts for the rest of the reasoning.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { call } from "../../api/client";
import { asAppError } from "../../api/errors";
import { Sheet } from "../../components/Sheet";
import { ObjectInspector } from "../../components/ObjectInspector";
import { useMessages } from "../../i18n/messages";
import { useVersionStore, type ConfigVersionSummary } from "../config-versions/versionStore";
import "./billing.css";
import {
  compareCatalogEntries,
  COMPARISON,
  formatCatalogEntries,
  formatCount,
  formatRate,
  formatTime,
  isEffective,
  isPolicyUnset,
  MAX_CATALOGS,
  MAX_ENTRIES,
  parseCatalogEntries,
  RATE_FIELDS,
  rateLabel,
  sortCatalogs,
  sourceLabel,
  WRITABLE_SOURCES,
  type Catalog,
  type CatalogEntry,
  type ImportReceipt,
  type PricePolicy,
} from "./model";

type ImportInput = Readonly<{
  catalog_version_id: string;
  effective_at_ms: number;
  source: string;
  entries: readonly CatalogEntry[];
}>;

function PolicyCard({
  catalogs,
  nowMs,
  editable,
  onError,
}: Readonly<{
  catalogs: readonly Catalog[];
  nowMs: number;
  editable: boolean;
  onError: (message: string) => void;
}>) {
  const queryClient = useQueryClient();
  const scope = useVersionStore((s) => s.context?.configVersionId);
  const [editing, setEditing] = useState(false);
  const [confirmClear, setConfirmClear] = useState(false);

  const policy = useQuery({
    queryKey: ["price-policy", scope],
    queryFn: () => call<PricePolicy>("getRoutingPricePolicy", {}, { versionScoped: true }),
    enabled: scope !== undefined,
    retry: false,
  });

  const invalidate = (): void => {
    void queryClient.invalidateQueries({ queryKey: ["price-policy", scope] });
  };

  const save = useMutation({
    mutationFn: (catalogVersionId: string) =>
      call<PricePolicy>(
        "setRoutingPricePolicy",
        { body: { catalog_version_id: catalogVersionId, comparison: COMPARISON } },
        { versionScoped: true, mutating: true },
      ),
    onSuccess: () => {
      setEditing(false);
      invalidate();
    },
    onError: (error) => onError(asAppError(error).message),
  });

  const clear = useMutation({
    mutationFn: () =>
      call<undefined>("clearRoutingPricePolicy", {}, { versionScoped: true, mutating: true }),
    onSuccess: () => {
      setConfirmClear(false);
      invalidate();
    },
    onError: (error) => onError(asAppError(error).message),
  });

  const unset = policy.isError && isPolicyUnset(asAppError(policy.error));
  const effective = catalogs.filter((catalog) => isEffective(catalog, nowMs));

  return (
    <div className="card bill-policy">
      <header className="page-head">
        <h3>路由价格策略</h3>
      </header>
      <p className="bill-note">选择用于比较路由成本的价格目录。计费用的价格目录在下方管理。</p>

      {policy.isPending ? (
        <p className="stat-sub">读取中…</p>
      ) : unset ? (
        <div className="empty-state" data-kind="empty">
          <p>
            本版本未配置价格策略。
          </p>
        </div>
      ) : policy.isError ? (
        <p role="alert" className="action-error">
          {asAppError(policy.error).message}
        </p>
      ) : (
        <table className="bill-kv">
          <tbody>
            <tr>
              <th scope="row">绑定目录</th>
              <td><ResourceIdentity id={policy.data.catalog_version_id} kind="catalog" /></td>
            </tr>
            <tr>
              <th scope="row">比较方式</th>
              <td className="mono">{policy.data.comparison}</td>
            </tr>
          </tbody>
        </table>
      )}

      <div className="bill-actions">
        <button
          type="button"
          disabled={!editable || effective.length === 0}
          title={editable ? undefined : "仅草稿版本可编辑"}
          onClick={() => setEditing(true)}
        >
          {unset ? "设置策略" : "改绑目录"}
        </button>
        {unset ? null : (
          <button
            type="button"
            className="danger"
            disabled={!editable}
            title={editable ? undefined : "仅草稿版本可编辑"}
            onClick={() => setConfirmClear(true)}
          >
            清除策略
          </button>
        )}
      </div>
      {effective.length === 0 ? (
        <p className="stat-sub">
          没有<strong>已生效</strong>的目录可绑定 —— 后端拒绝绑定生效时间在未来的目录
          (<span className="mono">RoutingPriceCatalogNotEffective</span>)。
        </p>
      ) : null}

      {editing ? (
        <Sheet title="绑定价格目录" onEscape={() => setEditing(false)}>
          <p className="stat-sub">
            只列出<strong>已生效</strong>的目录:生效时间在未来的目录后端会拒绝绑定。
            比较方式当前是闭集单值 <span className="mono">{COMPARISON}</span>。
          </p>
          <form
            className="sheet-form"
            onSubmit={(event: FormEvent<HTMLFormElement>) => {
              event.preventDefault();
              const id = String(new FormData(event.currentTarget).get("catalog_version_id") ?? "");
              if (id !== "") {
                save.mutate(id);
              }
            }}
          >
            <label>
              目录版本
              <select name="catalog_version_id" defaultValue={effective[0]?.catalog_version_id}>
                {effective.map((catalog) => (
                  <option key={catalog.catalog_version_id} value={catalog.catalog_version_id}>
                    {resourceName(catalog.catalog_version_id,"catalog")} · 生效 {formatTime(catalog.effective_at_ms)} ·{" "}
                    {formatCount(catalog.entries.length)} 条
                  </option>
                ))}
              </select>
            </label>
            <label>
              比较方式(契约当前唯一值)
              <input className="mono" value={COMPARISON} disabled />
            </label>
            <div className="sheet-actions">
              <button type="button" className="secondary" onClick={() => setEditing(false)}>
                取消
              </button>
              <button type="submit" disabled={save.isPending}>
                绑定
              </button>
            </div>
          </form>
        </Sheet>
      ) : null}

      {confirmClear ? (
        <Sheet title="确认清除价格策略" onEscape={() => setConfirmClear(false)}>
          <p className="reveal-warning">
            清除后,本配置版本的<strong>每一个候选</strong>在 Route Explain 里的{" "}
            <span className="mono">price_evidence</span> 都会变成{" "}
            <span className="mono">disabled</span>,基于费率的路由比较随之停止。
            目录本身不受影响。
          </p>
          <div className="sheet-actions">
            <button type="button" className="secondary" onClick={() => setConfirmClear(false)}>
              取消
            </button>
            <button
              type="button"
              className="danger"
              disabled={clear.isPending}
              onClick={() => clear.mutate()}
            >
              确认清除
            </button>
          </div>
        </Sheet>
      ) : null}
    </div>
  );
}

function ImportSheet({
  completed,
  saved,
  error,
  catalogs,
  initial,
  pending,
  onCancel,
  onInvalid,
  onSubmit,
}: Readonly<{
  completed:boolean;
  saved?: ImportReceipt;
  error?: unknown;
  catalogs: readonly Catalog[];
  initial: Readonly<{ id: string; entries: string }> | undefined;
  pending: boolean;
  onCancel: () => void;
  onInvalid: (message: string | undefined) => void;
  onSubmit: (input: ImportInput) => void;
}>) {
  const [catalogId]=useState(()=>`catalog-${crypto.randomUUID()}`);
  const [validation,setValidation]=useState<string>();
  const [prepared,setPrepared]=useState<ImportInput>();
  const [baselineId,setBaselineId]=useState(initial?.id??sortCatalogs(catalogs).find(c=>isEffective(c,Date.now()))?.catalog_version_id??"");
  const baseline=catalogs.find(c=>c.catalog_version_id===baselineId);
  const changes=prepared?compareCatalogEntries(baseline?.entries??[],prepared.entries):[];
  if(completed)return <Sheet title="价目表已导入" onEscape={onCancel}><p role="status">已保存 {saved?.entry_count??0} 条价格并应用配置。</p><p>生效时间：{formatTime(saved?.effective_at_ms??0)}</p><div className="sheet-actions"><button onClick={onCancel}>完成</button></div></Sheet>;
  return (
    <Sheet title={initial === undefined ? "导入价格目录" : "以现有目录为模板导入"} onEscape={()=>!pending&&onCancel()}>
      <p className="stat-sub">导入一份完整的新价目表；历史目录和账本保留。确认后对所有配置可见。</p>
      <label>对比目录<select value={baselineId} disabled={pending} onChange={event=>setBaselineId(event.target.value)}><option value="">不与现有目录对比</option>{sortCatalogs(catalogs).map(c=><option key={c.catalog_version_id} value={c.catalog_version_id}>{formatTime(c.effective_at_ms)} · {sourceLabel(c.source)}</option>)}</select></label>
      <form
        hidden={prepared!==undefined}
        className="sheet-form price-import-form"
        onSubmit={(event: FormEvent<HTMLFormElement>) => {
          event.preventDefault();
          const data = new FormData(event.currentTarget);
          const parsed = parseCatalogEntries(String(data.get("entries") ?? ""));
          if (!parsed.ok) {
            setValidation(parsed.reason);
            onInvalid(parsed.reason);
            return;
          }
          const effectiveAt = Date.parse(String(data.get("effective_at") ?? ""));
          if (!Number.isFinite(effectiveAt)) {
            setValidation("生效时间不是一个合法时刻。");
            onInvalid("生效时间不是一个合法时刻。");
            return;
          }
          setValidation(undefined);onInvalid(undefined);
          setPrepared({
            catalog_version_id: String(data.get("catalog_version_id") ?? "").trim(),
            effective_at_ms: effectiveAt,
            source: String(data.get("source") ?? "operator"),
            entries: parsed.entries,
          });
        }}
      >
        <input type="hidden" name="catalog_version_id" value={catalogId}/>
        <label>
          生效时间（本地时区）
          <input name="effective_at" type="datetime-local" required />
          <small>
            生效时间在未来的目录可以导入,但<strong>在到期之前不能绑定为路由价格策略</strong>。
          </small>
        </label>
        <label>
          来源
          <select name="source" defaultValue="operator">
            {WRITABLE_SOURCES.map((source) => (
              <option key={source} value={source}>
                {sourceLabel(source)}
              </option>
            ))}
          </select>
        </label>
        <label>
          条目(JSON 数组,1–{MAX_ENTRIES} 条)
          <textarea
            name="entries"
            className="mono bill-entries"
            rows={12}
            required
            defaultValue={initial?.entries ?? ""}
            placeholder={`[\n  {\n    "provider_id": "prov-a",\n    "channel_id": "ch-a",\n    "model": "minimax-m3",\n    "input_microunits_per_million": 1500000,\n    "output_microunits_per_million": 6000000,\n    "reasoning_microunits_per_million": 0,\n    "cache_read_microunits_per_million": 0,\n    "cache_creation_microunits_per_million": 0,\n    "cached_microunits_per_million": 0\n  }\n]`}
          />
          <small>
            六个费率字段单位是 <strong>microunits / 百万 token</strong>,必须是 ≥ 0 的整数。
            契约没有声明币种,本页不做任何折算。
          </small>
        </label>
        <div className="sheet-actions">
          <button type="button" className="secondary" disabled={pending} onClick={onCancel}>
            取消
          </button>
          <button type="submit" disabled={pending}>
            预览差异
          </button>
        </div>
      </form>
      {saved?<p role="status">价目表已保存；配置应用尚待确认，请勿重复导入。</p>:null}
      {validation?<p role="alert">{validation}</p>:null}
      {error?<p role="alert">{asAppError(error).message}</p>:null}
      {prepared?<div className="price-import-preview">
        <h3>确认价格变更</h3><p>生效时间：{formatTime(prepared.effective_at_ms)} · 共 {prepared.entries.length} 项</p>
        <p>新增 {changes.filter(c=>c.kind==="added").length} · 变价 {changes.filter(c=>c.kind==="changed").length} · 不再包含 {changes.filter(c=>c.kind==="removed").length}</p>
        {changes.length===0?<p>费率没有变化。</p>:changes.map((change,index)=>{const row=change.after??change.before!;return <details key={index}><summary>{row.model} · {({added:"新增",changed:"变价",removed:"新目录不再包含"})[change.kind]}</summary><p><ResourceIdentity id={row.provider_id} kind="upstream"/> · <ResourceIdentity id={row.channel_id} kind="endpoint"/></p><dl className="fact-grid">{RATE_FIELDS.map(field=><div key={field}><dt>{rateLabel(field)}</dt><dd>{change.before?formatRate(change.before[field]):"—"} → {change.after?formatRate(change.after[field]):"—"}</dd></div>)}</dl></details>})}
        <div className="sheet-actions"><button type="button" className="secondary" disabled={pending} onClick={()=>setPrepared(undefined)}>返回修改</button><button type="button" disabled={pending||saved!==undefined} onClick={()=>onSubmit(prepared)}>确认导入</button></div>
      </div>:null}
    </Sheet>
  );
}

export function BillingPage() {
  const t = useMessages();
  const queryClient = useQueryClient();
  const context = useVersionStore((s) => s.context);
  const scope = context?.configVersionId;
  const editable = context?.status !== "archived";

  const [notice, setNotice] = useState<string | undefined>();
  const [error, setError] = useState<string | undefined>();
  const [workingId,setWorkingId]=useState<string>();
  const [appliedVersion,setAppliedVersion]=useState<ConfigVersionSummary>();
  const [savedCatalog,setSavedCatalog]=useState<ImportReceipt>();
  const [importing, setImporting] = useState<
    Readonly<{ id: string; entries: string }> | undefined
  >();
  const [importOpen, setImportOpen] = useState(false);
  const [rollback, setRollback] = useState<Catalog | undefined>();
  const [expanded, setExpanded] = useState<string | undefined>();
  const [inspected, setInspected] = useState<Catalog>();

  const catalogs = useQuery({
    queryKey: ["billing-catalogs", scope],
    queryFn: () => call<readonly Catalog[]>("listBillingCatalogs", {}, { versionScoped: true }),
    enabled: scope !== undefined,
    retry: false,
  });

  const invalidate = (): void => {
    void queryClient.invalidateQueries({ queryKey: ["billing-catalogs", scope] });
  };

  const receipt = (result: ImportReceipt): void => {
    setImportOpen(false);
    setImporting(undefined);
    setRollback(undefined);
    setNotice(
      `目录 ${resourceName(result.catalog_version_id,"catalog")} 已${result.operation === "rolled_back" ? "回滚创建" : "导入"}` +
        `(${formatCount(result.entry_count)} 条` +
        `${result.rolled_back_from === null ? "" : `,复制自 ${resourceName(result.rolled_back_from,"catalog")}`})。` +
        `它对所有配置版本可见;要让路由用它,还需在上方绑定为价格策略。`,
    );
    invalidate();
  };

  const importCatalog = useMutation({
    mutationFn: async (input: ImportInput) => {
      const task=await beginConfigurationTask("导入价格目录");setWorkingId(task.version.id);
      const catalog=await task.mutate<ImportReceipt>("importBillingCatalog",{body:input});setSavedCatalog(catalog);
      return {catalog,version:await task.finish()};
    },
    onSuccess: ({catalog,version})=>{setAppliedVersion(version);setSavedCatalog(catalog);invalidate();},
    onError: (cause) => setError(asAppError(cause).message),
  });

  const rollbackCatalog = useMutation({
    mutationFn: async (input: Readonly<{ from: string; id: string; effective_at_ms: number }>) => {
      const task=await beginConfigurationTask("恢复价格目录");setWorkingId(task.version.id);
      const catalog=await task.mutate<ImportReceipt>("rollbackBillingCatalog",{path:{catalog_version_id:input.from},body:{new_catalog_version_id:input.id,effective_at_ms:input.effective_at_ms}});setSavedCatalog(catalog);
      return {catalog,version:await task.finish()};
    },
    onSuccess: ({catalog,version})=>{useVersionStore.getState().select(version);receipt(catalog);setSavedCatalog(undefined);setWorkingId(undefined);},
    onError: (cause) => setError(asAppError(cause).message),
  });

  if (scope === undefined) {
    return (
      <section>
        <h2>{t.nav.billing}</h2>
        <ProcessingStatus compact />
        <div className="card empty-state" data-kind="empty">
          <p>选择配置版本后维护该版本的价格策略。计费处理状态跨版本可读。</p>
        </div>
      </section>
    );
  }

  const rows = sortCatalogs(catalogs.data ?? []);
  const nowMs = Date.now();

  return (
    <section className="billing-page">
      <header className="page-head">
        <h2>{t.nav.billing}</h2>
      </header>
      <ProcessingStatus compact />

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

      <details className="card"><summary>高级路由价格策略</summary><PolicyCard catalogs={rows} nowMs={nowMs} editable={context?.status==="draft"} onError={setError} /></details>
      <ConfigurationTaskNotice workingId={workingId} error={importCatalog.error??rollbackCatalog.error} onReview={(version)=>{setImportOpen(false);setRollback(undefined);useVersionStore.getState().select(version);}}/>

      <div className="card bill-catalogs">
        <header className="page-head">
          <h3>
            价格目录 <span className="idchip mono">{formatCount(rows.length)}</span>
          </h3>
          <button
            type="button"
            disabled={!editable}
            title={editable ? undefined : "请返回当前配置后修改"}
            onClick={() => {
              setAppliedVersion(undefined);setSavedCatalog(undefined);setWorkingId(undefined);importCatalog.reset();
              setImporting(undefined);
              setImportOpen(true);
            }}
          >
            导入目录
          </button>
        </header>
        <p className="bill-note">导入新价目表更新费率，历史目录和账本保留。最多保存 {MAX_CATALOGS} 份。</p>

        {catalogs.isError ? (
          <div className="empty-state" data-kind="error">
            <p>{asAppError(catalogs.error).message}</p>
          </div>
        ) : catalogs.isPending ? (
          <p className="stat-sub">读取中…</p>
        ) : rows.length === 0 ? (
          <div className="empty-state" data-kind="empty">
            <p>还没有任何价格目录 —— 在此之前,所有计价都是 unpriced。</p>
          </div>
        ) : (
          <table className="bill-table">
            <thead>
              <tr>
                <th scope="col">目录版本</th>
                <th scope="col">生效时间(UTC)</th>
                <th scope="col">创建时间(UTC)</th>
                <th scope="col">来源</th>
                <th scope="col">条目</th>
                <th scope="col">操作</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((catalog) => (
                <tr key={catalog.catalog_version_id} data-future={isEffective(catalog, nowMs) ? undefined : "true"}>
                  <th scope="row" className="mono">
                    <ResourceIdentity id={catalog.catalog_version_id} kind="catalog" />
                  </th>
                  <td data-label="生效时间" className="mono">
                    {formatTime(catalog.effective_at_ms)}
                    {isEffective(catalog, nowMs) ? null : (
                      <span className="bill-future">未生效</span>
                    )}
                  </td>
                  <td data-label="创建时间" className="mono">{formatTime(catalog.created_at_ms)}</td>
                  <td data-label="来源">{sourceLabel(catalog.source)}</td>
                  <td data-label="价格条目" className="mono bill-num">{formatCount(catalog.entries.length)}</td>
                  <td className="row-actions">
                    <button className="secondary" onClick={() => setInspected(catalog)}>详情</button>
                    <button
                      type="button"
                      className="secondary"
                      onClick={() =>
                        setExpanded(
                          expanded === catalog.catalog_version_id
                            ? undefined
                            : catalog.catalog_version_id,
                        )
                      }
                    >
                      {expanded === catalog.catalog_version_id ? "收起" : "看条目"}
                    </button>
                    <button
                      type="button"
                      className="secondary"
                      disabled={!editable}
                      onClick={() => {
                        setAppliedVersion(undefined);setSavedCatalog(undefined);setWorkingId(undefined);importCatalog.reset();
                        setImporting({
                          id: catalog.catalog_version_id,
                          entries: formatCatalogEntries(catalog.entries),
                        });
                        setImportOpen(true);
                      }}
                    >
                      复制编辑
                    </button>
                    <button
                      type="button"
                      className="secondary"
                      disabled={!editable}
                      onClick={() => {setSavedCatalog(undefined);setWorkingId(undefined);rollbackCatalog.reset();setRollback(catalog);}}
                    >
                      恢复价格
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}

      {expanded === undefined ? null : (
          <div className="bill-entries-view">
            <h4>
              <span className="mono">{expanded}</span> 的条目 · 单位 microunits / 百万 token
            </h4>
            <div className="tablewrap">
              <table className="bill-table">
                <thead>
                  <tr>
                    <th scope="col">Provider</th>
                    <th scope="col">Channel</th>
                    <th scope="col">模型</th>
                    {RATE_FIELDS.map((field) => (
                      <th key={field} scope="col">
                        {rateLabel(field)}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {(rows.find((row) => row.catalog_version_id === expanded)?.entries ?? []).map(
                    (entry, index) => (
                      <tr key={`${entry.provider_id}/${entry.channel_id}/${entry.model}/${index}`}>
                        <td>{entry.provider_id ? <ResourceIdentity id={entry.provider_id} kind="upstream" /> : "—"}</td>
                        <td>{entry.channel_id ? <ResourceIdentity id={entry.channel_id} kind="endpoint" /> : "—"}</td>
                        <td className="mono">{entry.model}</td>
                        {RATE_FIELDS.map((field) => (
                          <td key={field} className="mono bill-num">
                            {formatRate(entry[field])}
                          </td>
                        ))}
                      </tr>
                    ),
                  )}
                </tbody>
              </table>
            </div>
          </div>
        )}
      </div>

      {inspected === undefined ? null : <ObjectInspector title={resourceName(inspected.catalog_version_id,"catalog")} scope="全局价格目录 · 单位 microunits / 百万 token" onClose={() => setInspected(undefined)} facts={[
        ["来源", sourceLabel(inspected.source)], ["生效时间", formatTime(inspected.effective_at_ms)],
        ["创建时间", formatTime(inspected.created_at_ms)], ["生效状态", isEffective(inspected, nowMs) ? "已生效" : "尚未生效"],
        ["价格条目", inspected.entries.length],
      ]}>
        <div className="price-evidence">{inspected.entries.map((entry, index) => <details key={index}>
          <summary>{entry.model}<span className="entity-meta"> · {entry.provider_id ? <ResourceIdentity id={entry.provider_id} kind="upstream" /> : "—"} / {entry.channel_id ? <ResourceIdentity id={entry.channel_id} kind="endpoint" /> : "—"}</span></summary>
          <dl className="fact-grid">{RATE_FIELDS.map((field) => <div key={field}><dt>{rateLabel(field)}</dt><dd>{formatRate(entry[field])}</dd></div>)}</dl>
        </details>)}</div>
      </ObjectInspector>}

      {importOpen ? (
        <ImportSheet
          completed={appliedVersion!==undefined}
          saved={savedCatalog}
          error={importCatalog.error}
          catalogs={catalogs.data??[]}
          initial={importing}
          pending={importCatalog.isPending}
          onCancel={() => {
            if(appliedVersion)useVersionStore.getState().select(appliedVersion);
            setImportOpen(false);
            setImporting(undefined);
          }}
          onInvalid={setError}
          onSubmit={(input) => importCatalog.mutate(input)}
        />
      ) : null}

      {rollback === undefined ? null : (
        <Sheet title={`回滚到 ${resourceName(rollback.catalog_version_id,"catalog")}`} onEscape={() => setRollback(undefined)}>
          <p className="stat-sub">
            回滚<strong>不会删除任何东西</strong>:它复制这份目录的条目,创建一个
            <strong>新的目录版本</strong>向前追加。旧目录与其间的目录都原样保留。
          </p>
          <form
            className="sheet-form"
            onSubmit={(event: FormEvent<HTMLFormElement>) => {
              event.preventDefault();
              const data = new FormData(event.currentTarget);
              const effectiveAt = Date.parse(String(data.get("effective_at") ?? ""));
              if (!Number.isFinite(effectiveAt)) {
                setError("生效时间不是一个合法时刻。");
                return;
              }
              rollbackCatalog.mutate({
                from: rollback.catalog_version_id,
                id: String(data.get("new_catalog_version_id") ?? "").trim(),
                effective_at_ms: effectiveAt,
              });
            }}
          >
            <label>
              新目录版本 ID
              <input name="new_catalog_version_id" className="mono" required maxLength={128} />
            </label>
            <label>
              生效时间（本地时区）
              <input name="effective_at" type="datetime-local" required />
            </label>
            <div className="sheet-actions">
              <button type="button" className="secondary" onClick={() => setRollback(undefined)}>
                取消
              </button>
              <button type="submit" disabled={rollbackCatalog.isPending||savedCatalog!==undefined}>
                创建回滚目录
              </button>
            </div>
          </form>
        </Sheet>
      )}
    </section>
  );
}
