// Shell: exactly three chrome glass panes — rail, topbar, (draft-only) dock.
// V6 keeps a shared frosted workspace beneath the three refractive chrome panes.
import { useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { Link, NavLink, Navigate, Outlet, useLocation } from "react-router-dom";
import { call } from "../api/client";
import { GlassSurface } from "../components/glass/GlassSurface";
import { PrismLens } from "../components/glass/PrismLens";
import {
  useVersionStore,
  type ConfigVersionSummary,
} from "../features/config-versions/versionStore";
import { useMessages } from "../i18n/messages";
import { useSessionStore } from "../session/sessionStore";
import { DraftDock } from "./DraftDock";
import { NAV_GROUPS, NAV_ITEMS, primaryRoute, workspacePages } from "./navigation";
import { resolvedTheme, useThemeStore } from "./themeStore";

/**
 * Resource pages are version-scoped, so the active configuration still needs
 * to be selected before they issue their reads. This is intentionally not
 * visual chrome: daily work should not be framed around an internal revision.
 */
function ConfigurationBootstrap() {
  const selectInitialActive = useVersionStore((s) => s.selectInitialActive);
  const versions = useQuery({
    queryKey: ["config-versions"],
    queryFn: () => call<ConfigVersionSummary[]>("listConfigVersions"),
    staleTime: 30_000,
  });

  useEffect(() => {
    if (versions.data !== undefined) selectInitialActive(versions.data);
  }, [selectInitialActive, versions.data]);

  if (!versions.isError) return null;
  return <div className="workspace-context" data-status="error" role="alert">
    <strong>无法读取配置上下文。</strong>
    <span>资源暂不可用；重新读取后会恢复当前活动配置。</span>
    <button type="button" className="secondary" onClick={() => void versions.refetch()}>重新读取</button>
  </div>;
}

export function AppShell() {
  const unlocked = useSessionStore((s) => s.unlocked);
  const passwordChangeRequired = useSessionStore((s) => s.passwordChangeRequired);
  const sessionGeneration = useSessionStore((s) => s.generation);
  const selectionGeneration = useVersionStore((s) => s.selectionGeneration);
  const context = useVersionStore((s) => s.context);
  const conflict = useVersionStore((s) => s.conflict);
  const clearConflict = useVersionStore((s) => s.clearConflict);
  const t = useMessages();
  const { pathname } = useLocation();
  const [menuOpen, setMenuOpen] = useState(false);
  const choice = useThemeStore((s) => s.choice);
  const setChoice = useThemeStore((s) => s.setChoice);
  const currentGroup = NAV_GROUPS.find((group) => group.items.some((item) => item.to === primaryRoute(pathname)));
  const currentPage = NAV_ITEMS.find((item) => item.to === pathname);
  const pages = workspacePages(pathname);
  const canvasRef = useRef<HTMLElement>(null);

  // The canvas — not the window — is the scroll container now (content slides
  // under the fixed glass chrome), so route changes must reset *its* offset.
  useEffect(() => {
    canvasRef.current?.scrollTo({ top: 0 });
    setMenuOpen(false);
  }, [pathname]);

  useEffect(() => {
    if (!menuOpen) return;
    const close = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setMenuOpen(false);
        document.getElementById("nav-toggle")?.focus();
      }
    };
    window.addEventListener("keydown", close);
    return () => window.removeEventListener("keydown", close);
  }, [menuOpen]);

  if (!unlocked || passwordChangeRequired) {
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
          <button className="nav-toggle secondary" id="nav-toggle" aria-label={t.navigation.menu}
            aria-controls="main-navigation" aria-expanded={menuOpen} onClick={() => setMenuOpen(!menuOpen)}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true"><path d="M4 6h16 M4 12h16 M4 18h16" /></svg>
          </button>
          <strong className="brand">
            <svg className="brandmark" viewBox="0 0 28 28" fill="none" aria-hidden="true"><path d="m14 2 12 12-12 12L2 14Z" fill="currentColor" fillOpacity=".12" stroke="currentColor" strokeWidth="1.5" /><path d="m14 6 8 8-8 8V6Z" fill="currentColor" fillOpacity=".35" /></svg>
            <span>Prism</span>
          </strong>
          <div className="top-context">
            {currentGroup === undefined ? null : <span>{t.navigation[currentGroup.label]} / </span>}
            <strong>{currentPage === undefined ? "Prism" : t.nav[currentPage.key]}</strong>
          </div>
          <button className="chrome-action secondary" aria-label={t.navigation.theme}
            onClick={() => setChoice(resolvedTheme(choice) === "dark" ? "light" : "dark")}><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true"><circle cx="12" cy="12" r="8" /><path d="M12 4a8 8 0 0 1 0 16Z" fill="currentColor" stroke="none" /></svg></button>
        </GlassSurface>

        {conflict ? (
          <div role="alert" className="conflict-bar">
            {t.version.conflict}
            <button type="button" onClick={clearConflict}>
              {t.version.conflictAck}
            </button>
          </div>
        ) : null}
      </div>

      <GlassSurface as="nav" className="rail" material={material} pane="rail" id="main-navigation" open={menuOpen}>
        {NAV_GROUPS.map((group) => (
          <div key={group.label} className="rail-group">
            <div className="rail-label">{t.navigation[group.label]}</div>
            {group.items.map((item) => (
              <Link
                key={item.to}
                to={item.to}
                aria-current={primaryRoute(pathname) === item.to ? "page" : undefined}
                className={primaryRoute(pathname) === item.to ? "on" : ""}
                onClick={() => setMenuOpen(false)}
              >
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d={item.icon} /></svg>
                <span>{t.nav[item.key]}</span>
              </Link>
            ))}
          </div>
        ))}
      </GlassSurface>

      <main className="canvas" ref={canvasRef} data-context-version={context?.configVersionId} data-context-status={context?.status}>
        <div className="workspace">
          <ConfigurationBootstrap />
          {context?.status === "active" || context === undefined ? null : (
            <div className="workspace-context" data-status={context.status} role="status">
              {context.status === "archived"
                ? "正在查看历史配置：资源为只读，无法保存或发布修改。"
                : "正在编辑草稿：未发布的修改不会影响当前服务。"}
              <Link to="/versions">查看配置上下文</Link>
            </div>
          )}
          {pages.length > 1 ? <nav className="workspace-navigation" aria-label="工作区页面">
            {pages.map((item) => <NavLink key={item.to} to={item.to} end>{item.to==="/models"?"已接入模型":item.to==="/catalog"?"上游模型":t.nav[item.key]}</NavLink>)}
          </nav> : null}
          <Outlet key={`${sessionGeneration}:${selectionGeneration}`} />
        </div>
      </main>

      <DraftDock key={`${sessionGeneration}:${context?.configVersionId ?? "none"}`} />
    </div>
  );
}
