/**
 * Static P10-03 management-shell entry point.
 *
 * This module deliberately does not construct a ManagementApi or make a network request. P10-04+
 * owns individual CRUD pages after matching protected HTTP routes exist. The generated client is
 * imported only to keep its compile-time contract visible to the SPA build.
 */
import type { ManagementOperationName } from "./generated/management-client.js";

type Workspace = {
  readonly name: string;
  readonly detail: string;
  readonly phase: string;
};

const workspaces: readonly Workspace[] = [
  {
    name: "Upstreams",
    detail: "Endpoints, credentials, discovery and bounded test workflows.",
    phase: "P10-04",
  },
  {
    name: "Routing",
    detail: "Public models, routes, candidates, access groups and client keys.",
    phase: "P10-05",
  },
  {
    name: "Runtime",
    detail: "Catalog, health, quota, route explanation and request tracing.",
    phase: "P10-06",
  },
  {
    name: "Configuration",
    detail: "Version validation, publication, rollback and operation audit.",
    phase: "P10-07",
  },
];

const generatedSurface: readonly ManagementOperationName[] = [];

function applicationMarkup(): string {
  const cards = workspaces
    .map(
      ({ name, detail, phase }) => `
        <article class="card">
          <h2>${name}</h2>
          <p>${detail}</p>
          <p class="status">Scheduled for ${phase}</p>
        </article>`,
    )
    .join("");

  return `
    <div class="shell">
      <aside class="sidebar">
        <p class="brand">CPA Rust Gateway<span>Management plane</span></p>
        <nav class="navigation" aria-label="Management sections">
          <a aria-current="page" href="#overview">Overview</a>
          <a href="#upstreams">Upstreams</a>
          <a href="#routing">Routing</a>
          <a href="#runtime">Runtime</a>
          <a href="#configuration">Configuration</a>
        </nav>
      </aside>
      <section class="content" id="overview">
        <div>
          <p class="eyebrow">Static management shell</p>
          <h1>Control plane is deliberately gated</h1>
        </div>
        <p class="lead">
          This isolated TypeScript SPA is built from the frozen management OpenAPI contract. It
          does not retain a Management Key, issue a request, or expose an unimplemented operation.
        </p>
        <p class="notice">
          The management HTTP listener and resource operations are not mounted by this build. A
          later task must explicitly connect each page through the P10-02 protected Scope.
        </p>
        <section class="cards" aria-label="Scheduled management workspaces">${cards}</section>
      </section>
    </div>`;
}

function mount(): void {
  const root = document.querySelector<HTMLElement>("#app");
  if (root === null) {
    throw new Error("management application root is missing");
  }
  root.innerHTML = applicationMarkup();
  void generatedSurface;
}

mount();
