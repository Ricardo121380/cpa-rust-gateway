import type { ComponentType } from "react";
import { Overview } from "@/features/overview/Overview";
import { Requests } from "@/features/requests/Requests";
import { Usage } from "@/features/usage/Usage";
import { Accounts } from "@/features/accounts/Accounts";
import { Onboarding } from "@/features/onboarding/Onboarding";
import { Upstreams } from "@/features/upstreams/Upstreams";
import { Routing } from "@/features/routing/Routing";
import { Access } from "@/features/access/Access";
import { Versions } from "@/features/versions/Versions";
import { Runtime } from "@/features/runtime/Runtime";
import { Audit } from "@/features/audit/Audit";
import { System } from "@/features/system/System";

// Static imports on purpose: the gateway embeds exactly main.js + vendor.js, so no route-level chunks.
export const PAGES: Record<string, ComponentType> = {
  overview: Overview, requests: Requests, usage: Usage, accounts: Accounts, onboarding: Onboarding,
  upstreams: Upstreams, routing: Routing, access: Access, versions: Versions, runtime: Runtime, audit: Audit, system: System,
};
