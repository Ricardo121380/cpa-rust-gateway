import { useState, type FormEvent } from "react";
import { KeySquare } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { send } from "@/api/hooks";
import { describeError } from "@/api/errors";
import { store, useSession } from "@/api/session";
import { toast } from "@/app/toast";

/** Required after an initial login (only password change and logout are allowed then), and from Settings. */
export function ChangePassword({ forced, onDone }: { forced?: boolean; onDone?: () => void }) {
  const session = useSession();
  const [cur, setCur] = useState("");
  const [next, setNext] = useState("");
  const [again, setAgain] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const tooShort = next.length > 0 && (next.length < 12 || next.length > 128);
  const mismatch = again.length > 0 && again !== next;

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (tooShort || mismatch || !next) return;
    setBusy(true); setError(null);
    try {
      await send("POST", "/admin/auth/password", { body: { current_password: cur, new_password: next } });
      if (session.kind === "admin") store.setSession({ ...session, mustChangePassword: false });
      toast({ title: "密码已更新", status: "success" });
      setCur(""); setNext(""); setAgain(""); onDone?.();
    } catch (err) { setError(describeError(err).title); } finally { setBusy(false); }
  }

  const form = (
    <form onSubmit={submit} className="grid gap-4">
      <div className="grid gap-1.5"><Label htmlFor="cp">当前密码</Label><Input id="cp" type="password" autoComplete="current-password" value={cur} onChange={(e) => setCur(e.target.value)} required /></div>
      <div className="grid gap-1.5"><Label htmlFor="np">新密码</Label><Input id="np" type="password" autoComplete="new-password" value={next} onChange={(e) => setNext(e.target.value)} aria-invalid={tooShort} required /><span className="text-xs" style={{ color: tooShort ? "var(--bad)" : "var(--muted-foreground)" }}>12–128 个字符</span></div>
      <div className="grid gap-1.5"><Label htmlFor="ap">再次输入新密码</Label><Input id="ap" type="password" autoComplete="new-password" value={again} onChange={(e) => setAgain(e.target.value)} aria-invalid={mismatch} required />{mismatch && <span className="text-xs" style={{ color: "var(--bad)" }}>两次输入不一致</span>}</div>
      {error && <p role="alert" className="text-sm" style={{ color: "var(--bad)" }}>{error}</p>}
      <div className="flex gap-2">{forced && <Button type="button" variant="outline" onClick={() => send("POST", "/admin/auth/logout").finally(() => store.signOut())}>退出</Button>}<Button type="submit" disabled={busy}><KeySquare />{busy ? "正在保存…" : "更新密码"}</Button></div>
    </form>
  );
  if (!forced) return form;
  return (
    <div className="grid min-h-screen place-items-center bg-background px-4">
      <div className="w-full max-w-[420px] rounded-[28px] border bg-card p-7 shadow-xl">
        <h1 className="text-lg font-bold">设置新密码</h1>
        <p className="mb-5 mt-1 text-sm text-muted-foreground">这是初始会话。更新密码之前，其他管理操作都不可用。</p>
        {form}
      </div>
    </div>
  );
}
