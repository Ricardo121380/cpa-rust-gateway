import { resourceName } from "../../utils/resourceNames";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { call } from "../../api/client";
import { Sheet } from "../../components/Sheet";
import { ReadStatus } from "../../components/ReadStatus";
import { useVersionStore, type ConfigVersionSummary } from "./versionStore";

type Audit = {
  id: number;
  action: string;
  config_version_id: string;
  replaced_config_version_id?: string | null;
};
export function LifecycleConfirmation({
  mode,
  id,
  pending,
  error,
  onCancel,
  onConfirm,
}: Readonly<{
  mode: "publish" | "rollback";
  id: string;
  pending: boolean;
  error?: string | undefined;
  onCancel: () => void;
  onConfirm: (expectedActive: string, lifecycleEvent: string) => void;
}>) {
  const context = useVersionStore((state) => state.context);
  const [revision] = useState(context?.revision);
  const query = useQuery({
    queryKey: ["lifecycle-confirmation", mode, id, revision],
    queryFn: async () => {
      const versions = await call<ConfigVersionSummary[]>("listConfigVersions");
      const audit = await call<Audit[]>("listManagementAuditEvents");
      return { versions, audit };
    },
    retry: false,
    staleTime: 0,
    refetchOnMount: "always",
  });
  const active = query.data?.versions.find(
    (version) => version.status === "active",
  );
  const target =
    mode === "publish"
      ? id
      : [...(query.data?.audit ?? [])]
          .filter(
            (event) =>
              event.config_version_id === active?.id &&
              ["config_published", "config_rolled_back"].includes(event.action),
          )
          .sort((a, b) => b.id - a.id)[0]?.replaced_config_version_id;
  const known = query.data?.versions.find((version) => version.id === id);
  const unchanged =
    context?.configVersionId === id &&
    context.revision === revision &&
    known?.revision === revision;
  const valid =
    unchanged &&
    (mode === "publish"
      ? known?.status === "draft"
      : active?.id === id && Boolean(target));
  return (
    <Sheet
      title={mode === "publish" ? "确认发布" : "确认回滚"}
      onEscape={pending ? () => {} : onCancel}
    >
      <ReadStatus
        pending={query.isPending}
        error={query.error}
        hasData={query.data !== undefined}
        retry={() => void query.refetch()}
      />
      <dl className="fact-grid">
        <div>
          <dt>当前活动版本</dt>
          <dd>{query.isPending ? "读取中…" : (active ? resourceName(active.id,"config",active.description) : "无")}</dd>
        </div>
        <div>
          <dt>{mode === "publish" ? "发布目标" : "回滚目标"}</dt>
          <dd>{target ? resourceName(target,"config",query.data?.versions.find(v=>v.id===target)?.description) : "无可用目标"}</dd>
        </div>
        <div>
          <dt>核对 revision</dt>
          <dd>{revision ?? "未选择版本"}</dd>
        </div>
      </dl>
      <p>
        操作会更新活动配置并写入审计。当前 serve
        进程需重启，才能装配切换后的数据面。
      </p>
      {query.data && !valid ? (
        <p role="alert">版本已变化或没有可用目标，请关闭后重新选择并核对。</p>
      ) : null}
      {error ? <p role="alert">{error}</p> : null}
      <div className="sheet-actions">
        <button className="secondary" disabled={pending} onClick={onCancel}>
          取消
        </button>
        <button
          disabled={
            pending ||
            query.isFetching ||
            query.isError ||
            Boolean(error) ||
            !query.data ||
            !valid
          }
          onClick={() =>
            onConfirm(
              JSON.stringify(active?.id ?? null),
              String(
                Math.max(
                  0,
                  ...(query.data?.audit ?? [])
                    .filter((event) =>
                      ["config_published", "config_rolled_back"].includes(
                        event.action,
                      ),
                    )
                    .map((event) => event.id),
                ),
              ),
            )
          }
        >
          {mode === "publish" ? "确认发布" : "确认回滚"}
        </button>
      </div>
    </Sheet>
  );
}
