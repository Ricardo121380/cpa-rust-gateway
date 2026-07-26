// Overview. Two truth layers, honestly separated (docs/07 §7.1):
//  - wiring scale: real counts from the existing list contracts, per version;
//  - observability: G2/G3 own the KPI tiles — until wired, a dedicated
//    "pipeline unwired" state, never fake zeros.
import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { call } from "../../api/client";
import { StatusBadge } from "../../components/StatusBadge";
import { messages } from "../../i18n/messages";
import {
  useVersionStore,
  type ConfigVersionSummary,
} from "../config-versions/versionStore";

function useCount(queryKey: string, operation: Parameters<typeof call>[0], scope: string | undefined) {
  return useQuery({
    queryKey: [queryKey, scope],
    queryFn: async () => ((await call<unknown[]>(operation, {}, { versionScoped: true })) ?? []).length,
    enabled: scope !== undefined,
    staleTime: 30_000,
  });
}

export function OverviewPage() {
  const context = useVersionStore((s) => s.context);
  const scope = context?.configVersionId;

  const versions = useQuery({
    queryKey: ["config-versions"],
    queryFn: () => call<ConfigVersionSummary[]>("listConfigVersions"),
    staleTime: 30_000,
  });
  const active = versions.data?.find((row) => row.status === "active");

  const upstreams = useCount("upstreams-count", "listUpstreams", scope);
  const egress = useCount("egress-count", "listEgressPolicies", scope);
  const models = useCount("models-count", "listPublicModels", scope);
  const groups = useCount("groups-count", "listAccessGroups", scope);
  const keys = useCount("keys-count", "listClientKeys", scope);

  const counts: ReadonlyArray<{ label: string; to: string; value: number | undefined }> = [
    { label: "上游", to: "/upstreams", value: upstreams.data },
    { label: "出口策略", to: "/egress", value: egress.data },
    { label: "公开模型", to: "/models", value: models.data },
    { label: "访问组", to: "/access", value: groups.data },
    { label: "Client Key", to: "/access", value: keys.data },
  ];

  return (
    <section>
      <h2>{messages.nav.overview}</h2>

      <div className="overview-grid">
        <div className="card">
          <h3>活动版本</h3>
          {active !== undefined ? (
            <p>
              <span className="mono">{active.id}</span> <StatusBadge status="active" />
              <br />
              <span className="idchip mono">{active.revision}</span>{" "}
              <span style={{ color: "var(--ink-2)", fontSize: 13 }}>{active.description}</span>
            </p>
          ) : (
            <p style={{ color: "var(--ink-2)" }}>尚无活动版本 —— 发布一个草稿后出现。</p>
          )}
          <Link to="/versions">前往配置版本 →</Link>
        </div>

        <div className="card">
          <h3>布线规模{scope === undefined ? "(未选择版本)" : ""}</h3>
          {scope === undefined ? (
            <p style={{ color: "var(--ink-2)" }}>在顶栏选择一个配置版本后显示。</p>
          ) : (
            <div className="count-row">
              {counts.map((item) => (
                <Link key={item.label} to={item.to} className="count-tile">
                  <span className="count-value mono">{item.value ?? "…"}</span>
                  <span className="count-label">{item.label}</span>
                </Link>
              ))}
            </div>
          )}
        </div>
      </div>

      <div className="card empty-state" data-kind="unwired" style={{ marginTop: 14 }}>
        <p>
          {messages.state.unwired}
          <br />
          <small style={{ color: "var(--ink-3)" }}>
            今日 KPI、流量趋势、健康条带与 Token 构成将在 G2(事件管道接线)与
            G3(分析端点)交付后点亮 —— 归后端会话。
          </small>
        </p>
      </div>
    </section>
  );
}

export function PlaceholderPage({ title }: Readonly<{ title: string }>) {
  return (
    <section>
      <h2>{title}</h2>
      <div className="card empty-state" data-kind="empty">
        <p>{messages.state.empty}</p>
      </div>
    </section>
  );
}
