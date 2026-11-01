import type { ComponentType } from "react";
import { Overview } from "@/features/overview/Overview";
import { PageHeader } from "@/components/kit";
import { NAV } from "./nav";

const Todo = (id: string) => () => <PageHeader title={NAV.find((n) => n.id === id)!.label} desc="施工中" />;
export const PAGES: Record<string, ComponentType> = Object.fromEntries(NAV.map((n) => [n.id, Todo(n.id)]));
PAGES.overview = Overview;
