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
import { useMessages } from "../../i18n/messages";
import { useVersionStore } from "../config-versions/versionStore";
import "./billing.css";
import {
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
        <code className="idchip mono">getRoutingPricePolicy</code>
      </header>
      <p className="bill-note">
        这一项<strong>属于当前配置版本</strong>(与下方全局的目录不同)。它决定 Route Explain
        与调度看到的价格证据来自哪一份目录。<strong>未配置时,每个候选的{" "}
        <span className="mono">price_evidence</span> 都是 <span className="mono">disabled</span></strong>
        —— 那不是"没有价格",而是"没有做比较"。
      </p>

      {policy.isPending ? (
        <p className="stat-sub">读取中…</p>
      ) : unset ? (
        <div className="empty-state" data-kind="empty">
          <p>
            本版本未配置价格策略。
            <br />
            <small className="muted-3">
              404 <span className="mono">management_resource_not_found</span> 在这里是一个正常状态,
              不是错误。
            </small>
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
              <td className="mono">{policy.data.catalog_version_id}</td>
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
                    {catalog.catalog_version_id} · 生效 {formatTime(catalog.effective_at_ms)} ·{" "}
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
  initial,
  pending,
  onCancel,
  onInvalid,
  onSubmit,
}: Readonly<{
  initial: Readonly<{ id: string; entries: string }> | undefined;
  pending: boolean;
  onCancel: () => void;
  onInvalid: (message: string) => void;
  onSubmit: (input: ImportInput) => void;
}>) {
  return (
    <Sheet title={initial === undefined ? "导入价格目录" : "以现有目录为模板导入"} onEscape={onCancel}>
      <p className="stat-sub">
        导入是<strong>整份提交</strong>,不是增量:这里的条目就是新目录的<strong>全部</strong>内容。
        目录<strong>只能新增</strong> —— 契约没有修改或删除算子。
        <br />
        新目录对<strong>所有配置版本</strong>立即可见;它不属于当前草稿。
      </p>
      <form
        className="sheet-form"
        onSubmit={(event: FormEvent<HTMLFormElement>) => {
          event.preventDefault();
          const data = new FormData(event.currentTarget);
          const parsed = parseCatalogEntries(String(data.get("entries") ?? ""));
          if (!parsed.ok) {
            onInvalid(parsed.reason);
            return;
          }
          const effectiveAt = Date.parse(String(data.get("effective_at") ?? ""));
          if (!Number.isFinite(effectiveAt)) {
            onInvalid("生效时间不是一个合法时刻。");
            return;
          }
          onSubmit({
            catalog_version_id: String(data.get("catalog_version_id") ?? "").trim(),
            effective_at_ms: effectiveAt,
            source: String(data.get("source") ?? "operator"),
            entries: parsed.entries,
          });
        }}
      >
        <label>
          目录版本 ID
          <input name="catalog_version_id" className="mono" required maxLength={128} />
        </label>
        <label>
          生效时间(UTC)
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
                {source}
              </option>
            ))}
          </select>
          <small>读模型里还有 test,但契约不接受它作为写入值。</small>
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
          <button type="button" className="secondary" onClick={onCancel}>
            取消
          </button>
          <button type="submit" disabled={pending}>
            导入
          </button>
        </div>
      </form>
    </Sheet>
  );
}

export function BillingPage() {
  const t = useMessages();
  const queryClient = useQueryClient();
  const context = useVersionStore((s) => s.context);
  const scope = context?.configVersionId;
  const editable = context?.status === "draft";

  const [notice, setNotice] = useState<string | undefined>();
  const [error, setError] = useState<string | undefined>();
  const [importing, setImporting] = useState<
    Readonly<{ id: string; entries: string }> | undefined
  >();
  const [importOpen, setImportOpen] = useState(false);
  const [rollback, setRollback] = useState<Catalog | undefined>();
  const [expanded, setExpanded] = useState<string | undefined>();

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
      `目录 ${result.catalog_version_id} 已${result.operation === "rolled_back" ? "回滚创建" : "导入"}` +
        `(${formatCount(result.entry_count)} 条` +
        `${result.rolled_back_from === null ? "" : `,复制自 ${result.rolled_back_from}`})。` +
        `它对所有配置版本可见;要让路由用它,还需在上方绑定为价格策略。`,
    );
    invalidate();
  };

  const importCatalog = useMutation({
    mutationFn: (input: ImportInput) =>
      call<ImportReceipt>(
        "importBillingCatalog",
        { body: input },
        { versionScoped: true, mutating: true },
      ),
    onSuccess: receipt,
    onError: (cause) => setError(asAppError(cause).message),
  });

  const rollbackCatalog = useMutation({
    mutationFn: (input: Readonly<{ from: string; id: string; effective_at_ms: number }>) =>
      call<ImportReceipt>(
        "rollbackBillingCatalog",
        {
          path: { catalog_version_id: input.from },
          body: { new_catalog_version_id: input.id, effective_at_ms: input.effective_at_ms },
        },
        { versionScoped: true, mutating: true },
      ),
    onSuccess: receipt,
    onError: (cause) => setError(asAppError(cause).message),
  });

  if (scope === undefined) {
    return (
      <section>
        <h2>{t.nav.billing}</h2>
        <div className="card empty-state" data-kind="empty">
          <p>先在顶栏选择一个配置版本 —— 价格策略属于版本,读取目录也要带上它。</p>
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
        <code className="idchip mono">listBillingCatalogs</code>
      </header>

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

      <PolicyCard catalogs={rows} nowMs={nowMs} editable={editable} onError={setError} />

      <div className="card bill-catalogs">
        <header className="page-head">
          <h3>
            价格目录 <span className="idchip mono">{formatCount(rows.length)}</span>
          </h3>
          <button
            type="button"
            disabled={!editable}
            title={editable ? undefined : "仅草稿版本可编辑"}
            onClick={() => {
              setImporting(undefined);
              setImportOpen(true);
            }}
          >
            导入目录
          </button>
        </header>
        <p className="bill-note">
          目录是<strong>全局</strong>的,不属于任何配置版本 —— 顶栏选哪个版本都看到同一份清单,
          导入也会立刻对所有版本可见。上限 {MAX_CATALOGS} 份。
          <br />
          没有修改与删除算子:改价的做法是<strong>导入一份新目录</strong>,
          撤销的做法是<strong>回滚出一份新目录</strong>(复制旧内容,向前追加,不删除历史)。
        </p>

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
                    {catalog.catalog_version_id}
                  </th>
                  <td className="mono">
                    {formatTime(catalog.effective_at_ms)}
                    {isEffective(catalog, nowMs) ? null : (
                      <span className="bill-future">未生效</span>
                    )}
                  </td>
                  <td className="mono">{formatTime(catalog.created_at_ms)}</td>
                  <td>{sourceLabel(catalog.source)}</td>
                  <td className="mono bill-num">{formatCount(catalog.entries.length)}</td>
                  <td className="row-actions">
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
                        setImporting({
                          id: catalog.catalog_version_id,
                          entries: formatCatalogEntries(catalog.entries),
                        });
                        setImportOpen(true);
                      }}
                    >
                      当模板
                    </button>
                    <button
                      type="button"
                      className="secondary"
                      disabled={!editable}
                      onClick={() => setRollback(catalog)}
                    >
                      回滚到它
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
                        <td className="mono">{entry.provider_id}</td>
                        <td className="mono">{entry.channel_id}</td>
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

      {importOpen ? (
        <ImportSheet
          initial={importing}
          pending={importCatalog.isPending}
          onCancel={() => {
            setImportOpen(false);
            setImporting(undefined);
          }}
          onInvalid={setError}
          onSubmit={(input) => importCatalog.mutate(input)}
        />
      ) : null}

      {rollback === undefined ? null : (
        <Sheet title={`回滚到 ${rollback.catalog_version_id}`} onEscape={() => setRollback(undefined)}>
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
              生效时间(UTC)
              <input name="effective_at" type="datetime-local" required />
            </label>
            <div className="sheet-actions">
              <button type="button" className="secondary" onClick={() => setRollback(undefined)}>
                取消
              </button>
              <button type="submit" disabled={rollbackCatalog.isPending}>
                创建回滚目录
              </button>
            </div>
          </form>
        </Sheet>
      )}
    </section>
  );
}
