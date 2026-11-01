// Account onboarding, one flow per channel family, Perch-style rows:
//   Codex / Claude  → PKCE (start → open authorization URL → paste callback → credential saved)
//   Kiro / Kimi     → device code (start → show user code → poll at the server's interval)
//   Grok (native)   → native device authorization (POST /admin/native-account-authorizations → poll)
//   API key         → account import (secret is write-only, encrypted at rest)
// PKCE, device-code and import flows create credentials in the working draft.
import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Check, ChevronRight, Copy, ExternalLink, KeyRound, Plug, ShieldCheck } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { ensureDraft, send, useGet, type S } from "@/api/hooks";
import { describeError } from "@/api/errors";
import { toast } from "@/app/toast";
import { go } from "@/app/router";
import { CatDot, Mono, PageHeader, Panel, Q, Status } from "@/components/kit";
import { useParam } from "@/components/table";
import { categoryOfKind } from "@/lib/labels";

type Flow = "pkce" | "device" | "native" | "import";
const FLOW: Record<string, { flow: Flow; label: string; desc: string }> = {
  codex: { flow: "pkce", label: "OAuth 授权（PKCE）", desc: "在浏览器里登录 ChatGPT 账号，然后把回调地址粘贴回来。" },
  claude: { flow: "pkce", label: "OAuth 授权（PKCE）", desc: "在浏览器里登录 Claude 账号，然后把回调地址粘贴回来。" },
  kiro: { flow: "device", label: "设备码授权", desc: "在 AWS Builder ID 页面输入设备码，完成后自动保存。" },
  kimi: { flow: "device", label: "设备码授权", desc: "在 Kimi 页面输入设备码，完成后自动保存。" },
  grok: { flow: "native", label: "原生设备授权", desc: "在 xAI 账号页面输入设备码；身份由 OAuth 提供方返回，不从名称推断。" },
  api: { flow: "import", label: "导入 API Key", desc: "密钥加密落盘，管理面永远不会再返回明文。" },
};
const CHANNELS = ["openai-compatible", "anthropic-compatible", "codex", "claude", "grok.official", "kiro", "kimi-api", "kimi-coding"];
const newId = (cat: string) => `cr-${cat}-${Date.now().toString(36).slice(-5)}`;

export function Onboarding() {
  const upstreams = useGet<S<"Upstream">[]>(["upstreams"], "/admin/upstreams");
  const reauth = useParam("reauth");
  const [active, setActive] = useState<S<"Upstream"> | null>(null);
  useEffect(() => {
    if (!reauth || !upstreams.data) return;
    const cat = new URLSearchParams(location.hash.split("?")[1]).get("category");
    const u = upstreams.data.find((x) => categoryOfKind(x.kind) === cat);
    if (u) setActive(u);
  }, [reauth, upstreams.data]);
  return (
    <>
      <PageHeader title="接入账号" desc="按上游选择接入方式。授权、设备码和导入都写入当前草稿，发布后才参与调度。" />
      <Panel flush>
        <Q q={upstreams} what="上游列表">{(list) => (
          <ul className="divide-y">{list.map((u) => {
            const cat = categoryOfKind(u.kind); const f = FLOW[cat] ?? FLOW.api;
            return (
              <li key={u.id} className="flex flex-wrap items-center gap-x-5 gap-y-3 px-6 py-4 sm:flex-nowrap">
                <CatDot category={cat} size={40} />
                <div className="min-w-0 flex-1"><div className="flex items-center gap-2 text-[15px] font-semibold">{u.name}{!u.enabled && <Badge variant="outline">已停用</Badge>}</div><div className="mt-0.5 text-[13px] text-muted-foreground">{f.desc}</div></div>
                <div className="hidden text-right text-xs text-muted-foreground md:block"><div>{f.label}</div><Mono>{u.kind}</Mono></div>
                <Button onClick={() => setActive(u)} disabled={!u.enabled}><Plug />接入<ChevronRight className="-mr-1" /></Button>
              </li>
            );
          })}</ul>
        )}</Q>
      </Panel>
      <p className="flex gap-2 text-xs text-muted-foreground"><ShieldCheck className="size-4 shrink-0" />控制台不会读取或缓存授权码、令牌和密钥；关闭页面会丢弃未完成的授权会话。</p>
      {active && <FlowDialog upstream={active} reauth={reauth || undefined} onClose={() => setActive(null)} />}
    </>
  );
}

type Device = { state: string; session_id?: string; user_code?: string; verification_uri?: string; interval_ms?: number; retry_at_ms?: number; credential_id?: string; identity?: S<"AccountDisplayIdentity"> | null };

function FlowDialog({ upstream, reauth, onClose }: { upstream: S<"Upstream">; reauth?: string; onClose: () => void }) {
  const qc = useQueryClient();
  const cat = categoryOfKind(upstream.kind);
  const flow = (FLOW[cat] ?? FLOW.api).flow;
  const [step, setStep] = useState<1 | 2 | 3>(1);
  const [id, setId] = useState(reauth ?? newId(cat));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [oauth, setOauth] = useState<S<"OAuthOperation"> | null>(null);
  const [callback, setCallback] = useState("");
  const [device, setDevice] = useState<Device | null>(null);
  const [secret, setSecret] = useState("");
  const [channel, setChannel] = useState(cat === "api" ? "openai-compatible" : CHANNELS.find((c) => c.startsWith(cat)) ?? "openai-compatible");
  const [result, setResult] = useState<string | null>(null);
  const timer = useRef<number>(0);
  useEffect(() => () => clearTimeout(timer.current), []);
  const path = { upstream_id: upstream.id };
  const kind = cat === "kimi" ? "kimi" : "kiro";

  async function run(fn: () => Promise<void>) {
    setBusy(true); setError(null);
    try { await fn(); } catch (e) { setError(describeError(e).title); } finally { setBusy(false); }
  }
  const finish = (credential: string) => { setResult(credential); setStep(3); qc.invalidateQueries({ queryKey: ["inventory"] }); toast({ title: `已接入 ${credential}`, description: "写入当前草稿，发布后参与调度", status: "success" }); };

  const start = () => run(async () => {
    if (flow !== "native") await ensureDraft();
    if (flow === "pkce") {
      const op = await send<S<"OAuthOperation">>("POST", `/admin/upstreams/{upstream_id}/${cat === "claude" ? "claude" : "codex"}-authorization/start` as never, { path, body: { id, replace_existing: !!reauth } });
      setOauth(op); setStep(2);
    } else if (flow === "device") {
      const d = await send<Device>("POST", `/admin/upstreams/{upstream_id}/${kind}-authorization/start` as never, { path, body: { id, replace_existing: !!reauth } });
      setDevice(d); setStep(2); schedulePoll(d);
    } else if (flow === "native") {
      const d = await send<Device>("POST", "/admin/native-account-authorizations", { body: { name: id } });
      setDevice(d); setStep(2); schedulePoll(d);
    } else {
      const c = await send<S<"Credential">>("POST", "/admin/upstreams/{upstream_id}/account-import", { path, body: { id, channel, secret } });
      setSecret(""); finish(c.id);
    }
  });
  function schedulePoll(d: Device) {
    clearTimeout(timer.current);
    const wait = d.retry_at_ms ? Math.max(1000, d.retry_at_ms - Date.now()) : d.interval_ms ?? 5000;
    timer.current = window.setTimeout(async () => {
      try {
        const n = flow === "native"
          ? await send<Device>("POST", "/admin/native-account-authorizations/{session_id}/poll", { path: { session_id: d.session_id! } })
          : await send<Device>("POST", `/admin/upstreams/{upstream_id}/${kind}-authorization/poll` as never, { path, body: { id, session_id: d.session_id } });
        setDevice({ ...d, ...n });
        if (n.state === "pending") schedulePoll({ ...d, ...n });
        else if (n.state === "complete" || n.state === "completed") finish(n.credential_id ?? n.identity?.email ?? id);
        else setError(`授权结束：${n.state}`);
      } catch (e) { setError(describeError(e).title); }
    }, Math.min(wait, 15000));
  }
  const complete = () => run(async () => {
    const isUrl = /^https?:\/\//.test(callback.trim());
    const c = await send<S<"Credential">>("POST", `/admin/upstreams/{upstream_id}/${cat === "claude" ? "claude" : "codex"}-authorization/callback` as never, { path, body: { id, callback: isUrl ? { callback_url: callback.trim() } : { code: callback.trim(), state: undefined }, replace_existing: !!reauth } });
    setCallback(""); finish(c.id);
  });
  async function cancel() {
    clearTimeout(timer.current);
    try {
      if (step === 2 && flow === "pkce") await send("POST", `/admin/upstreams/{upstream_id}/${cat === "claude" ? "claude" : "codex"}-authorization/cancel` as never, { path, body: { id } });
      if (step === 2 && flow === "device" && device?.session_id) await send("POST", `/admin/upstreams/{upstream_id}/${kind}-authorization/cancel` as never, { path, body: { id, session_id: device.session_id } });
      if (step === 2 && flow === "native" && device?.session_id) await send("DELETE", "/admin/native-account-authorizations/{session_id}", { path: { session_id: device.session_id } });
    } catch { /* cancellation is best effort; the session also expires on its own */ }
    onClose();
  }
  const copy = (t: string) => navigator.clipboard?.writeText(t).then(() => toast({ title: "已复制", status: "success", duration: 1600 }));

  return (
    <Dialog open onOpenChange={(o) => !o && cancel()}>
      <DialogContent className="overflow-hidden sm:max-w-[540px]">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2"><CatDot category={cat} size={24} />{reauth ? "重新授权" : "接入"} {upstream.name}</DialogTitle>
          <DialogDescription>{(FLOW[cat] ?? FLOW.api).label}</DialogDescription>
        </DialogHeader>
        <ol className="flex items-center gap-2 text-xs" aria-label="步骤">{["填写", flow === "import" ? "导入" : "授权", "完成"].map((l, i) => <li key={l} className="flex items-center gap-2"><span className={`grid size-5 place-items-center rounded-full text-[10px] font-bold ${step > i ? "bg-primary text-primary-foreground" : "bg-muted text-muted-foreground"}`}>{step > i + 1 ? <Check className="size-3" /> : i + 1}</span><span className={step === i + 1 ? "font-semibold" : "text-muted-foreground"}>{l}</span>{i < 2 && <span className="h-px w-6 bg-border" />}</li>)}</ol>
        <div className="t-page-slide" data-page={String(step)}>
          <section className="t-page grid gap-3" data-page-id="1" inert={step !== 1} aria-hidden={step !== 1}>
            <div className="grid gap-1.5"><Label htmlFor="cid">凭据 ID</Label><Input id="cid" value={id} disabled={!!reauth} onChange={(e) => setId(e.target.value)} /><span className="text-xs text-muted-foreground">{reauth ? "替换这个凭据的授权，保留绑定和停用状态。" : "在配置图里唯一，之后用于绑定端点。"}</span></div>
            {flow === "import" && <>
              <div className="grid gap-1.5"><Label>渠道</Label><Select value={channel} onValueChange={setChannel}><SelectTrigger className="w-full"><SelectValue /></SelectTrigger><SelectContent>{CHANNELS.map((c) => <SelectItem key={c} value={c}>{c}</SelectItem>)}</SelectContent></Select></div>
              <div className="grid gap-1.5"><Label htmlFor="sec">密钥</Label><Textarea id="sec" value={secret} onChange={(e) => setSecret(e.target.value)} placeholder="写入后不可读取" className="font-mono text-xs" autoComplete="off" spellCheck={false} /><span className="flex items-center gap-1.5 text-xs text-muted-foreground"><KeyRound className="size-3" />XChaCha20-Poly1305 加密落盘</span></div>
            </>}
          </section>
          <section className="t-page grid gap-3" data-page-id="2" inert={step !== 2} aria-hidden={step !== 2}>
            {flow === "pkce" && oauth && <>
              <p className="text-sm">1. 打开授权地址并登录；2. 登录后浏览器会跳转到一个无法打开的地址，复制完整地址粘贴到下面。</p>
              <div className="flex items-center gap-2 rounded-lg bg-muted p-2 pl-3"><Mono className="flex-1 truncate text-xs">{oauth.authorization_url}</Mono><Button size="sm" variant="ghost" onClick={() => copy(oauth.authorization_url ?? "")}><Copy />复制</Button><Button size="sm" variant="ghost" asChild><a href={oauth.authorization_url ?? "#"} target="_blank" rel="noreferrer noopener"><ExternalLink />打开</a></Button></div>
              <div className="grid gap-1.5"><Label htmlFor="cb">回调地址或授权码</Label><Textarea id="cb" value={callback} onChange={(e) => setCallback(e.target.value)} className="font-mono text-xs" placeholder="http://localhost:1455/auth/callback?code=…&state=…" spellCheck={false} /></div>
              <span className="text-xs text-muted-foreground"><Status map="oauth" k={oauth.state} /> {oauth.expires_at_ms ? `· ${Math.max(0, Math.round((oauth.expires_at_ms - Date.now()) / 60000))} 分钟内有效` : ""}</span>
            </>}
            {(flow === "device" || flow === "native") && device && <>
              <p className="text-sm">在授权页面输入下面的设备码。完成后这里会自动进入下一步。</p>
              <div className="grid place-items-center gap-3 rounded-2xl border bg-muted/40 py-6">
                <button onClick={() => copy(device.user_code ?? "")} className="font-mono text-3xl font-bold tracking-[0.25em] hover:opacity-80" title="点击复制">{device.user_code}</button>
                <Button size="sm" variant="outline" asChild><a href={device.verification_uri ?? "#"} target="_blank" rel="noreferrer noopener"><ExternalLink />打开授权页面</a></Button>
              </div>
              <span className="t-shimmer text-xs" data-text="正在等待授权，按服务端给出的间隔轮询…">正在等待授权，按服务端给出的间隔轮询…</span>
            </>}
          </section>
          <section className="t-page grid place-items-center gap-3 py-4 text-center" data-page-id="3" inert={step !== 3} aria-hidden={step !== 3}>
            <span className="t-success-check" data-state={step === 3 ? "in" : "out"} style={{ color: "var(--ok)" }}><svg viewBox="0 0 48 48" width="44" height="44" fill="none"><circle cx="24" cy="24" r="20" stroke="currentColor" strokeWidth="3" opacity=".25" /><path d="M14 25l7 7 13-15" stroke="currentColor" strokeWidth="4" strokeLinecap="round" strokeLinejoin="round" /></svg></span>
            <b>已接入 <Mono>{result}</Mono></b>
            <p className="max-w-[40ch] text-xs text-muted-foreground">凭据已写入草稿。到「上游与端点」绑定到端点，再验证并发布草稿。</p>
          </section>
        </div>
        {error && <p role="alert" className="t-shake rounded-lg px-3 py-2 text-sm" style={{ background: "color-mix(in srgb, var(--bad) 10%, transparent)", color: "var(--bad)" }}>{error}</p>}
        <DialogFooter>
          {step < 3 ? <Button variant="outline" onClick={cancel}>取消</Button> : <Button variant="outline" onClick={() => { onClose(); go("upstreams"); }}>去绑定端点</Button>}
          {step === 1 && <Button disabled={busy || !id || (flow === "import" && !secret)} onClick={start}>{busy ? "正在处理…" : flow === "import" ? "导入" : "开始授权"}</Button>}
          {step === 2 && flow === "pkce" && <Button disabled={busy || !callback.trim()} onClick={complete}>{busy ? "正在保存…" : "完成授权"}</Button>}
          {step === 3 && <Button onClick={onClose}>完成</Button>}
        </DialogFooter>
        <p className="sr-only" aria-live="polite">{step === 3 ? `已接入 ${result}` : ""}</p>
      </DialogContent>
    </Dialog>
  );
}
