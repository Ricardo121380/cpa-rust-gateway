import { useMemo, type ReactNode } from "react";
import { GitFork, LogOut, Moon, Search, Send, Sun, UserRound } from "lucide-react";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { BounceSidebar, type BounceSidebarItem } from "@/components/ui/bounce-sidebar";
import { NotificationBell } from "@/components/ui/notification-bell";
import { CommandPalette, type CommandItem } from "@/components/motion/command-palette";
import { forkDraft, send, useGet, type S } from "@/api/hooks";
import { store, useConfigContext, useSession } from "@/api/session";
import { go, useRoute } from "./router";
import { NAV } from "./nav";
import { toggleTheme, useTheme } from "./theme";
import { ConfigBar } from "./ConfigBar";
import { Logo } from "./Logo";
import { PAGES } from "./pages";

const SIDEBAR: { item: BounceSidebarItem; id?: string }[] = [];
NAV.forEach((n, i) => { if (i === 0 || NAV[i - 1].group !== n.group) SIDEBAR.push({ item: { label: n.group, heading: true } }); SIDEBAR.push({ item: n.label, id: n.id }); });

export function Shell() {
  const { page } = useRoute();
  const theme = useTheme();
  const session = useSession();
  const ctx = useConfigContext();
  const availability = useGet<S<"RuntimeAvailability">>(["availability"], "/admin/runtime/availability", undefined, { graph: false, refetchInterval: 30_000 });
  const issues = (availability.data ?? []).filter((a) => a.availability !== "available").length;
  const current = NAV.find((n) => n.id === page) ?? NAV[0];
  const Page = PAGES[current.id];

  const commands: CommandItem[] = useMemo(() => [
    ...NAV.map((n) => ({ id: "go-" + n.id, label: n.label, group: "跳转", icon: n.icon, keywords: n.keywords, onSelect: () => go(n.id) })),
    { id: "act-fork", label: "从生效版本创建草稿", group: "配置", icon: GitFork, onSelect: () => ctx.active && forkDraft(ctx.active, "控制台编辑") },
    { id: "act-publish", label: "验证并发布当前草稿", group: "配置", icon: Send, onSelect: () => go("versions", { publish: 1 }) },
    { id: "act-theme", label: theme === "dark" ? "切换到明亮主题" : "切换到暗色主题", group: "偏好", icon: theme === "dark" ? Sun : Moon, onSelect: () => toggleTheme() },
    { id: "act-logout", label: "退出登录", group: "偏好", icon: LogOut, onSelect: () => logout() },
  ], [theme, ctx.active]);

  async function logout() {
    if (session.kind === "admin") await send("POST", "/admin/auth/logout").catch(() => undefined);
    store.signOut();
  }

  return (
    <div className="grid min-h-screen lg:grid-cols-[232px_minmax(0,1fr)]">
      <a href="#main" onClick={(e) => { e.preventDefault(); document.getElementById("main")?.focus(); }} className="sr-only z-50 rounded-lg bg-primary px-3 py-2 text-primary-foreground focus:not-sr-only focus:fixed focus:left-3 focus:top-3">跳到主内容</a>
      <aside className="sticky top-0 hidden h-screen flex-col border-r bg-sidebar px-3 py-4 lg:flex" aria-label="主导航">
        <a href="#/overview" className="mb-5 flex items-center gap-2.5 px-2"><Logo size={30} /><div className="leading-tight"><b className="tracking-[0.12em]">CPAR</b><div className="text-[11px] text-muted-foreground">网关控制台</div></div></a>
        <BounceSidebar items={SIDEBAR.map((s) => s.item)} value={SIDEBAR.findIndex((s) => s.id === current.id)} onChange={(i) => SIDEBAR[i].id && go(SIDEBAR[i].id!)} dotColor="var(--c2)" className="flex-1 overflow-y-auto [&_a]:text-[13.5px]" />
        <div className="mt-3 rounded-xl border bg-card p-3 text-xs">
          <div className="flex items-center gap-2"><span className="grid size-7 place-items-center rounded-lg bg-muted"><UserRound className="size-3.5" /></span><div className="min-w-0 flex-1"><b className="block truncate">{session.kind === "admin" ? session.username : "管理 Key"}</b><span className="text-muted-foreground">{session.kind === "admin" ? `会话至 ${new Date(session.expiresAt).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", hour12: false })}` : "CLI 凭据"}</span></div><Button size="icon" variant="ghost" className="size-7" aria-label="退出登录" onClick={logout}><LogOut className="size-3.5" /></Button></div>
        </div>
      </aside>
      <div className="min-w-0">
        <header className="sticky top-0 z-30 flex h-14 items-center gap-3 border-b bg-background/80 px-4 backdrop-blur-md lg:px-6">
          <a href="#/overview" className="lg:hidden" aria-label="总览"><Logo size={26} /></a>
          <div className="hidden min-w-0 text-sm md:block"><span className="text-muted-foreground">{current.group} / </span><b>{current.label}</b></div>
          <div className="ml-auto flex min-w-0 items-center gap-1.5">
            <ConfigBar />
            <CommandPalette items={commands} placeholder="跳转到页面或执行操作" emptyMessage="没有匹配项" />
            <button onClick={() => document.dispatchEvent(new KeyboardEvent("keydown", { key: "k", metaKey: true, bubbles: true }))} className="hidden items-center gap-2 rounded-lg border bg-card px-2.5 py-1.5 text-xs text-muted-foreground hover:text-foreground xl:flex" aria-label="打开命令面板"><Search className="size-3.5" />搜索<kbd className="ml-4 rounded border px-1.5 font-mono text-[10px]">⌘K</kbd></button>
            <NotificationBell count={issues} color="red" size={18} aria-label={`${issues} 个账号目标不可调度`} title={`${issues} 个账号目标不可调度`} onClick={() => go("runtime")} />
            <Button size="icon" variant="ghost" aria-label="切换主题" onClick={(e) => toggleTheme(e)}>{theme === "dark" ? <Moon /> : <Sun />}</Button>
            <DropdownMenu>
              <DropdownMenuTrigger asChild><Button size="icon" variant="ghost" className="lg:hidden" aria-label="账户"><UserRound /></Button></DropdownMenuTrigger>
              <DropdownMenuContent align="end"><DropdownMenuLabel>{session.kind === "admin" ? session.username : "管理 Key"}</DropdownMenuLabel><DropdownMenuSeparator /><DropdownMenuItem onSelect={logout}><LogOut />退出登录</DropdownMenuItem></DropdownMenuContent>
            </DropdownMenu>
          </div>
        </header>
        <nav className="flex gap-1 overflow-x-auto border-b px-3 py-2 [scrollbar-width:none] lg:hidden" aria-label="主导航">
          {NAV.map((n) => <Button key={n.id} size="sm" variant={n.id === current.id ? "secondary" : "ghost"} onClick={() => go(n.id)}><n.icon />{n.label}</Button>)}
        </nav>
        <main id="main" tabIndex={-1} className="mx-auto max-w-[1400px] px-4 py-6 outline-none lg:px-8">
          {/* Enter-only page transition with Transitions.dev page tokens (250ms, smooth-out, 8px, 3px blur).
              Pure CSS keyframes: no exit phase and no dependence on animation frames from JS. */}
          <div key={current.id} className="page-enter" onAnimationEnd={(e) => { if (e.target === e.currentTarget) document.getElementById("main")?.focus({ preventScroll: true }); }}>
            <PageFrame><Page /></PageFrame>
          </div>
        </main>
      </div>
    </div>
  );
}
function PageFrame({ children }: { children: ReactNode }) { return <div className="[&>*+*]:mt-4">{children}</div>; }
