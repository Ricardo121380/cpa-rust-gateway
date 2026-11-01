import type { ComponentType } from "react";
import { Overview } from "@/features/overview/Overview";
import { Requests } from "@/features/requests/Requests";
import { Usage } from "@/features/usage/Usage";
import { Accounts } from "@/features/accounts/Accounts";
import { Onboarding } from "@/features/onboarding/Onboarding";
import { PageHeader } from "@/components/kit";
import { NAV } from "./nav";

const Todo = (id: string) => () => <PageHeader title={NAV.find((n) => n.id === id)!.label} desc="施工中" />;
export const PAGES: Record<string, ComponentType> = Object.fromEntries(NAV.map((n) => [n.id, Todo(n.id)]));
PAGES.overview = Overview;
PAGES.requests = Requests;
PAGES.usage = Usage;
PAGES.accounts = Accounts;
PAGES.onboarding = Onboarding;
