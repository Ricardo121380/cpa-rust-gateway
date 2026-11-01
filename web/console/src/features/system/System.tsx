import { Moon, Sun } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { useGet, type S } from "@/api/hooks";
import { useConfigContext, useSession } from "@/api/session";
import { toggleTheme, useTheme } from "@/app/theme";
import { KV, Mono, PageHeader, Panel, Q, Tonal } from "@/components/kit";
import { fmt } from "@/lib/format";
import { ChangePassword } from "@/features/auth/ChangePassword";

export function System() {
  const sys = useGet<S<"SystemInformation">>(["system"], "/admin/system", undefined, { graph: false, refetchInterval: 60_000 });
  const session = useSession();
  const ctx = useConfigContext();
  const theme = useTheme();
  return (
    <>
      <PageHeader title="系统" desc="网关构建信息与本地偏好。偏好只保存在当前页面内存中。" />
      <div className="grid gap-4 xl:grid-cols-2">
        <Panel title="网关" desc={<code>GET /admin/system</code>}>
          <Q q={sys} what="系统信息">{(s) => <KV rows={[["版本", <Mono>{s.build.version}</Mono>], ["构建修订", <Mono>{s.build.build_revision}</Mono>], ["目标平台", <Mono>{s.build.build_target}</Mono>], ["Rust", <Mono>{s.build.rust_version}</Mono>], ["存储 schema", s.build.schema_version], ["运行时长", fmt.uptime(s.uptime_seconds)], ["配置应用", <Mono>{s.configuration_application}</Mono>], ["接收请求", s.accepting_requests == null ? "未知" : s.accepting_requests ? <Tonal tone="ok">是</Tonal> : <Tonal tone="bad">否</Tonal>], ["实例", s.server_instance ? <Mono>{s.server_instance}</Mono> : "—"]]} />}</Q>
        </Panel>
        <Panel title="会话">
          <KV rows={[["身份", session.kind === "admin" ? <span>{session.username} <Badge variant="outline">管理员会话</Badge></span> : <Badge variant="outline">管理 Key</Badge>], ["会话到期", session.kind === "admin" ? fmt.date(session.expiresAt) : "随 Key 有效"], ["凭据保存", <span>只在内存 <Badge variant="outline">刷新即清除</Badge></span>], ["生效配置", <Mono>{ctx.active}</Mono>], ["当前查看", <Mono>{ctx.working} · {ctx.revision}</Mono>]]} />
        </Panel>
        <Panel title="外观">
          <div className="flex items-center justify-between gap-4"><div><b className="flex items-center gap-2">{theme === "dark" ? <Moon className="size-4" /> : <Sun className="size-4" />}暗色主题</b><p className="text-xs text-muted-foreground">默认跟随系统；切换时从开关位置展开</p></div><Switch checked={theme === "dark"} onCheckedChange={() => toggleTheme()} aria-label="暗色主题" /></div>
        </Panel>
        {session.kind === "admin" && <Panel title="修改密码" desc="修改后所有管理员会话都会失效，需要用新密码重新登录"><ChangePassword /></Panel>}
      </div>
    </>
  );
}
