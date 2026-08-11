import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./design/tokens.css";
import "./design/glass.css";
import "./design/modal.css";
import "./app/app.css";

const container = document.getElementById("app");
if (container === null) {
  throw new Error("missing #app container");
}
createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
