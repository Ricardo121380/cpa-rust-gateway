import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createHashRouter, RouterProvider } from "react-router-dom";
import { AppShell } from "./app/AppShell";
import { AccessPage } from "./features/access/AccessPage";
import { AuditBackupPage } from "./features/audit/AuditBackupPage";
import { VersionsPage } from "./features/config-versions/VersionsPage";
import { EgressPage } from "./features/egress/EgressPage";
import { ModelsPage } from "./features/models/ModelsPage";
import { OverviewPage, PlaceholderPage } from "./features/overview/OverviewPage";
import { UnlockPage } from "./features/unlock/UnlockPage";
import { UpstreamsPage } from "./features/upstreams/UpstreamsPage";
import { messages } from "./i18n/messages";

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
      { path: "usage", element: <PlaceholderPage title={messages.nav.usage} /> },
      { path: "monitoring", element: <PlaceholderPage title={messages.nav.monitoring} /> },
      { path: "versions", element: <VersionsPage /> },
      { path: "upstreams", element: <UpstreamsPage /> },
      { path: "models", element: <ModelsPage /> },
      { path: "access", element: <AccessPage /> },
      { path: "egress", element: <EgressPage /> },
      { path: "runtime", element: <PlaceholderPage title={messages.nav.runtime} /> },
      { path: "audit", element: <AuditBackupPage /> },
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
