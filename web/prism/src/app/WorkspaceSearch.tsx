import { useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { Sheet } from "../components/Sheet";
import { useOperationBoundary } from "../components/OperationBoundary";
import { useMessages } from "../i18n/messages";
import { NAV_ITEMS } from "./navigation";

/** Local navigation only: never searches or sends account/secret content. */
export function WorkspaceSearch() {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const trigger = useRef<HTMLButtonElement>(null);
  const admission = useOperationBoundary();
  const t = useMessages();
  const close = () => { setOpen(false); setQuery(""); };
  useEffect(() => {
    const shortcut = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== "k" || document.querySelector('[role="dialog"]')) return;
      event.preventDefault();
      admission.request(() => setOpen(true));
    };
    window.addEventListener("keydown", shortcut);
    return () => window.removeEventListener("keydown", shortcut);
  }, [admission]);
  return <>
    <button ref={trigger} className="workspace-search secondary" aria-label="搜索工作区" onClick={() => admission.request(() => setOpen(true))}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true"><circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 5 5"/></svg>
      <span>搜索工作区</span><kbd>⌘ K</kbd>
    </button>
    {open ? <Sheet title="搜索工作区" onEscape={close} guardUnsaved={false} returnFocus={() => trigger.current}>
      <label className="workspace-search-field"><span className="sr-only">工作区名称</span><input data-sheet-initial-focus value={query} onChange={event => setQuery(event.target.value)} placeholder="输入页面名称" /></label>
      <nav className="workspace-search-results" aria-label="搜索结果">
        {NAV_ITEMS.filter(item => t.nav[item.key].toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())).map(item => <Link key={item.to} to={item.to} onClick={close}>{t.nav[item.key]}<span aria-hidden="true">→</span></Link>)}
      </nav>
    </Sheet> : null}
  </>;
}
