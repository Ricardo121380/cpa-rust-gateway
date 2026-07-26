// Shell: exactly three chrome glass panes — rail, topbar, (draft-only) dock.
// Content canvas is always solid (docs/07 §5.2).
import { useQuery } from "@tanstack/react-query";
import { useEffect, useRef } from "react";
import { NavLink, Navigate, Outlet, useLocation } from "react-router-dom";
import { call } from "../api/client";
import { GlassSurface } from "../components/glass/GlassSurface";
import { PrismLens } from "../components/glass/PrismLens";
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
  const { pathname } = useLocation();
  const canvasRef = useRef<HTMLElement>(null);

  // The canvas — not the window — is the scroll container now (content slides
  // under the fixed glass chrome), so route changes must reset *its* offset.
  useEffect(() => {
    canvasRef.current?.scrollTo({ top: 0 });
  }, [pathname]);

  if (!unlocked) {
    return <Navigate to="/unlock" replace />;
  }

  const material = context?.status ?? "active";
  // Mirrors DraftDock's own render condition. Drives the canvas bottom
  // clearance so the floating dock can never cover the last card.
  const docked = context !== undefined && context.status === "draft";

  return (
    <div
      className="shell"
      data-conflict={conflict ? "true" : undefined}
      data-dock={docked ? "true" : undefined}
    >
      {/* Ambient layer: glass with nothing behind it cannot look like glass.
          Authored CSS gradients + an SVG grain, no images (CSP-clean). */}
      <div className="ambient" aria-hidden="true" />
      <div className="ambient-grain" aria-hidden="true">
        <svg aria-hidden="true" focusable="false">
          <rect width="100%" height="100%" filter="url(#prism-grain)" />
        </svg>
      </div>
      <PrismLens />

      <div className="topdeck">
        <GlassSurface as="header" className="topbar" material={material} pane="topbar">
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
      </div>

      <GlassSurface as="nav" className="rail" material={material} pane="rail">
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

      <main className="canvas" ref={canvasRef}>
        <Outlet />
      </main>

      <DraftDock />
    </div>
  );
}
