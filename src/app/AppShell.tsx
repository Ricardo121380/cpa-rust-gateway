// Shell: exactly three chrome glass panes — rail, topbar, (draft-only) dock.
// Content canvas is always solid (docs/07 §5.2).
import { useQuery } from "@tanstack/react-query";
import { NavLink, Navigate, Outlet } from "react-router-dom";
import { call } from "../api/client";
import { GlassSurface } from "../components/glass/GlassSurface";
import {
  useVersionStore,
  type ConfigVersionSummary,
} from "../features/config-versions/versionStore";
import { messages } from "../i18n/messages";
import { useSessionStore } from "../session/sessionStore";
import { DraftDock } from "./DraftDock";

const NAV_GROUPS: ReadonlyArray<ReadonlyArray<{ to: string; label: string }>> = [
  [
    { to: "/", label: messages.nav.overview },
    { to: "/usage", label: messages.nav.usage },
    { to: "/monitoring", label: messages.nav.monitoring },
  ],
  [
    { to: "/versions", label: messages.nav.versions },
    { to: "/upstreams", label: messages.nav.upstreams },
    { to: "/models", label: messages.nav.models },
    { to: "/access", label: messages.nav.access },
    { to: "/egress", label: messages.nav.egress },
  ],
  [
    { to: "/runtime", label: messages.nav.runtime },
    { to: "/audit", label: messages.nav.audit },
  ],
];

function VersionPicker() {
  const context = useVersionStore((s) => s.context);
  const select = useVersionStore((s) => s.select);
  const versions = useQuery({
    queryKey: ["config-versions"],
    queryFn: () => call<ConfigVersionSummary[]>("listConfigVersions"),
    staleTime: 30_000,
  });

  return (
    <label className="version-picker">
      <span className="visually-hidden">配置版本</span>
      <select
        className="mono"
        value={context?.configVersionId ?? ""}
        onChange={(event) => {
          const found = versions.data?.find((v) => v.id === event.target.value);
          if (found !== undefined) {
            select(found);
          }
        }}
      >
        <option value="" disabled>
          {messages.version.none}
        </option>
        {(versions.data ?? []).map((version) => (
          <option key={version.id} value={version.id}>
            {version.id} · {version.status} · {version.revision}
          </option>
        ))}
      </select>
    </label>
  );
}

export function AppShell() {
  const unlocked = useSessionStore((s) => s.unlocked);
  const context = useVersionStore((s) => s.context);
  const conflict = useVersionStore((s) => s.conflict);
  const clearConflict = useVersionStore((s) => s.clearConflict);

  if (!unlocked) {
    return <Navigate to="/unlock" replace />;
  }

  const material = context?.status ?? "active";

  return (
    <div className="shell">
      <GlassSurface as="header" className="topbar" material={material}>
        <strong className="brand">
          ◇ <span>Prism</span>
        </strong>
        <VersionPicker />
        {context !== undefined ? (
          <span className="idchip mono">{context.revision}</span>
        ) : null}
        {context !== undefined && context.status !== "draft" ? (
          <span className="readonly-note">{messages.version.readOnly}</span>
        ) : null}
      </GlassSurface>

      {conflict ? (
        <div role="alert" className="conflict-bar">
          {messages.version.conflict}
          <button type="button" onClick={clearConflict}>
            知道了
          </button>
        </div>
      ) : null}

      <GlassSurface as="nav" className="rail">
        {NAV_GROUPS.map((group, index) => (
          <div key={index} className="rail-group">
            {group.map((item) => (
              <NavLink
                key={item.to}
                to={item.to}
                end={item.to === "/"}
                className={({ isActive }) => (isActive ? "on" : "")}
              >
                {item.label}
              </NavLink>
            ))}
          </div>
        ))}
      </GlassSurface>

      <main className="canvas">
        <Outlet />
      </main>

      <DraftDock />
    </div>
  );
}
