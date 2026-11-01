import "@/styles/globals.css";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "@/app/App";
import { useTransport } from "@/api/client";

// Mock mode is resolved at build time; production builds drop the import entirely.
if (import.meta.env.VITE_CONSOLE_MOCK === "1") {
  const { mockFetch } = await import("@/api/mock/server");
  useTransport((input) => mockFetch(input as Request));
}

createRoot(document.getElementById("root")!).render(<StrictMode><App /></StrictMode>);
