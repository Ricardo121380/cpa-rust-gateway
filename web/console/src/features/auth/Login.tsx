import { useEffect, useState, type FormEvent } from "react";
import { KeyRound, LogIn, Moon, ShieldCheck, Sun, UserRound } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { api, unwrap } from "@/api/client";
import { describeError } from "@/api/errors";
import { CSRF_PATTERN, KEY_PATTERN, store } from "@/api/session";
import type { S } from "@/api/hooks";
import { toggleTheme, useTheme } from "@/app/theme";
import { Logo } from "@/app/Logo";

export function Login() {
  const [mode, setMode] = useState<"admin" | "key">("admin");
  const [user, setUser] = useState("admin");
  const [pass, setPass] = useState("");
  const [key, setKey] = useState("");
  const [csrf, setCsrf] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<{ title: string; detail?: string } | null>(null);
  const theme = useTheme();
  // Demo builds only: ?autologin signs in against the mock so page reviews skip the form.
  useEffect(() => {
    if (import.meta.env.VITE_CONSOLE_MOCK === "1" && location.search.includes("autologin")) {
      unwrap<S<"AdministratorSession">>(api.POST("/admin/auth/login", { body: { username: "admin", password: "demo" } }) as never).then((s) => store.setSession({ kind: "admin", username: s.username, token: s.session_token, csrf: s.csrf_token, expiresAt: s.expires_at_ms, mustChangePassword: false }));
    }
  }, []);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setError(null);
    if (mode === "key") {
      if (!KEY_PATTERN.test(key.trim())) return setError({ title: "管理 Key 以 mgmt_ 开头，长度 37–517 个字符。" });
      if (csrf && !CSRF_PATTERN.test(csrf.trim())) return setError({ title: "CSRF 令牌以 csrf_ 开头。" });
      store.setSession({ kind: "key", token: key.trim(), csrf: csrf.trim() || null });
      return;
    }
    setBusy(true);
    try {
      const s = await unwrap<S<"AdministratorSession">>(api.POST("/admin/auth/login", { body: { username: user, password: pass } }) as never);
      store.setSession({ kind: "admin", username: s.username, token: s.session_token, csrf: s.csrf_token, expiresAt: s.expires_at_ms, mustChangePassword: s.password_change_required });
    } catch (err) {
      setError(describeError(err));
    } finally {
      setBusy(false);
      setPass("");
    }
  }

  return (
    <div className="relative grid min-h-screen place-items-center overflow-hidden bg-background px-4">
      <div aria-hidden className="login-glow" />
      <Button variant="ghost" size="icon" className="absolute right-4 top-4" aria-label="切换主题" onClick={(e) => toggleTheme(e)}>{theme === "dark" ? <Moon /> : <Sun />}</Button>
      <form onSubmit={submit} className="relative z-10 w-full max-w-[400px] rounded-[28px] border bg-card/90 p-7 shadow-xl backdrop-blur-xl">
        <div className="mb-6 flex items-center gap-3"><Logo size={36} /><div><div className="text-lg font-bold tracking-wide">CPAR 控制台</div><div className="text-xs text-muted-foreground">网关管理与观测</div></div></div>
        <Tabs value={mode} onValueChange={(v) => { setMode(v as "admin" | "key"); setError(null); }} className="mb-5">
          <TabsList className="w-full"><TabsTrigger value="admin"><UserRound />管理员账号</TabsTrigger><TabsTrigger value="key"><KeyRound />管理 Key</TabsTrigger></TabsList>
        </Tabs>
        {mode === "admin" ? (
          <div className="grid gap-4">
            <div className="grid gap-1.5"><Label htmlFor="u">用户名</Label><Input id="u" autoComplete="username" value={user} onChange={(e) => setUser(e.target.value)} required /></div>
            <div className="grid gap-1.5"><Label htmlFor="p">密码</Label><Input id="p" type="password" autoComplete="current-password" value={pass} onChange={(e) => setPass(e.target.value)} required autoFocus /></div>
          </div>
        ) : (
          <div className="grid gap-4">
            <div className="grid gap-1.5"><Label htmlFor="k">管理 Key</Label><Input id="k" type="password" autoComplete="off" placeholder="mgmt_…" value={key} onChange={(e) => setKey(e.target.value)} required autoFocus /></div>
            <div className="grid gap-1.5"><Label htmlFor="c">CSRF 令牌 <span className="font-normal text-muted-foreground">（浏览器内修改配置时需要）</span></Label><Input id="c" type="password" autoComplete="off" placeholder="csrf_…" value={csrf} onChange={(e) => setCsrf(e.target.value)} /></div>
          </div>
        )}
        {error && <div role="alert" className="t-shake mt-4 rounded-xl px-3 py-2.5 text-sm" style={{ background: "color-mix(in srgb, var(--bad) 10%, transparent)", color: "var(--bad)" }}><b className="block">{error.title}</b>{error.detail && <span className="font-mono text-xs opacity-80">{error.detail}</span>}</div>}
        <Button type="submit" className="mt-6 w-full" size="lg" disabled={busy}><LogIn />{busy ? "正在登录…" : "登录"}</Button>
        <p className="mt-5 flex gap-2 text-xs text-muted-foreground"><ShieldCheck className="mt-0.5 size-3.5 shrink-0" />凭据只保存在当前页面的内存中，刷新或关闭页面后需要重新登录。</p>
        {import.meta.env.VITE_CONSOLE_MOCK === "1" && <p className="mt-2 rounded-lg bg-muted px-3 py-2 text-xs text-muted-foreground">演示模式：用户名 admin，任意密码。输入 change-me-now 可体验强制改密。</p>}
      </form>
    </div>
  );
}
