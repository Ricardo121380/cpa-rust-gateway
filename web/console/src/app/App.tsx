import { useEffect } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useSession } from "@/api/session";
import { ApiError } from "@/api/errors";
import { Toasts } from "./toast";
import { Shell } from "./Shell";
import { Login } from "@/features/auth/Login";
import { ChangePassword } from "@/features/auth/ChangePassword";
import { useTheme } from "./theme";

const qc = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 15_000,
      refetchOnWindowFocus: false,
      // Retrying a denial, a conflict or a disabled feature only repeats the same answer.
      retry: (n, e) => n < 2 && !(e instanceof ApiError && [0, 400, 401, 403, 404, 409, 503].includes(e.status) && e.code !== "management_operations_busy"),
    },
  },
});

export function App() {
  const session = useSession();
  useTheme();
  useEffect(() => { if (session.kind === "none") qc.clear(); }, [session.kind]);
  return (
    <QueryClientProvider client={qc}>
      <TooltipProvider delayDuration={250}>
        {session.kind === "none" ? <Login /> : session.kind === "admin" && session.mustChangePassword ? <ChangePassword forced /> : <Shell />}
        <Toasts />
      </TooltipProvider>
    </QueryClientProvider>
  );
}
