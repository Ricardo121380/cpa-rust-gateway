import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createHashRouter, RouterProvider } from "react-router-dom";
import { AppShell } from "./app/AppShell";
import { AccessPage } from "./features/access/AccessPage";
import { AuditBackupPage } from "./features/audit/AuditBackupPage";
import { VersionsPage } from "./features/config-versions/VersionsPage";
import { EgressPage } from "./features/egress/EgressPage";
import { ModelsPage } from "./features/models/ModelsPage";
import { MonitoringPage } from "./features/monitoring/MonitoringPage";
import { OverviewPage } from "./features/overview/OverviewPage";
import { RuntimePage } from "./features/runtime/RuntimePage";
import { SettingsPage } from "./features/settings/SettingsPage";
import { UnlockPage } from "./features/unlock/UnlockPage";
import { UpstreamsPage } from "./features/upstreams/UpstreamsPage";
import { UsagePage } from "./features/usage/UsagePage";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 1, refetchOnWindowFocus: false },
  },
});

const router = createHashRouter([
  { path: "/unlock", element: <UnlockPage /> },
  {
    path: "/",
    element: <AppShell />,
    children: [
      { index: true, element: <OverviewPage /> },
      { path: "usage", element: <UsagePage /> },
      { path: "monitoring", element: <MonitoringPage /> },
      { path: "versions", element: <VersionsPage /> },
      { path: "upstreams", element: <UpstreamsPage /> },
      { path: "models", element: <ModelsPage /> },
      { path: "access", element: <AccessPage /> },
      { path: "egress", element: <EgressPage /> },
      { path: "runtime", element: <RuntimePage /> },
      { path: "audit", element: <AuditBackupPage /> },
      { path: "settings", element: <SettingsPage /> },
    ],
  },
]);

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  );
}
