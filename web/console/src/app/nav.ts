import { Activity, BarChart3, Boxes, FileClock, GitBranch, HeartPulse, LayoutGrid, List, Plug, Server, Settings, Shield, Users, type LucideIcon } from "lucide-react";

export type NavItem = { id: string; label: string; group: string; icon: LucideIcon; keywords?: string[] };
export const NAV: NavItem[] = [
  { id: "overview", label: "总览", group: "观测", icon: LayoutGrid },
  { id: "requests", label: "请求", group: "观测", icon: List, keywords: ["attempt", "日志"] },
  { id: "usage", label: "用量与账单", group: "观测", icon: BarChart3, keywords: ["billing", "cost", "token", "价格"] },
  { id: "accounts", label: "账号池", group: "账号", icon: Users, keywords: ["credential", "quota", "配额", "凭据"] },
  { id: "onboarding", label: "接入账号", group: "账号", icon: Plug, keywords: ["oauth", "codex", "claude", "grok", "kiro", "kimi", "import"] },
  { id: "upstreams", label: "上游与端点", group: "配置", icon: Server, keywords: ["endpoint", "egress", "proxy", "出口"] },
  { id: "routing", label: "模型与路由", group: "配置", icon: GitBranch, keywords: ["model", "route", "candidate", "alias"] },
  { id: "access", label: "访问控制", group: "配置", icon: Shield, keywords: ["client key", "access group"] },
  { id: "versions", label: "配置版本", group: "配置", icon: Boxes, keywords: ["draft", "publish", "rollback", "发布"] },
  { id: "runtime", label: "运行时", group: "治理", icon: HeartPulse, keywords: ["availability", "catalog", "metrics"] },
  { id: "audit", label: "审计与备份", group: "治理", icon: FileClock, keywords: ["backup", "restore"] },
  { id: "system", label: "系统", group: "治理", icon: Settings, keywords: ["settings", "password", "theme"] },
];
export const ICON_ACTIVITY = Activity;
