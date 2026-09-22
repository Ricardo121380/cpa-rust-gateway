import { NavLink, useLocation } from "react-router-dom";
import { useMessages } from "../i18n/messages";
import { workspacePages } from "./navigation";

/** Keep the same routes and navigation guards; put workspace tabs below the page heading. */
export function WorkspaceTabs() {
  const { pathname } = useLocation();
  const t = useMessages();
  const pages = workspacePages(pathname);
  if (pages.length < 2) return null;
  const names: Record<string, string> = { "/models": "已开放模型", "/catalog": "上游目录", "/usage": "用量分析", "/billing": "价格目录", "/settings": "通用" };
  return <nav className="workspace-navigation" aria-label="工作区页面">{pages.map(item => <NavLink key={item.to} to={item.to} end>{names[item.to] ?? t.nav[item.key]}</NavLink>)}</nav>;
}
